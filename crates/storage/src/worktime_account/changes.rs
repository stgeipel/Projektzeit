use chrono::{Datelike, NaiveDate};
use projektzeit_core::{
    Timestamp,
    worktime_account::{Account, AccountError, Observations},
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{BalanceState, Change, Impact, Preview, Settings, StoreError, queries};

pub(super) fn revision(db: &Connection) -> Result<i64, StoreError> {
    Ok(db.query_row(
        "SELECT coalesce(max(id), 0) FROM account_audit_events",
        [],
        |row| row.get(0),
    )?)
}

pub(super) fn preview(
    db: &Connection,
    mut change: Change,
    as_of: NaiveDate,
    today: NaiveDate,
    observed_at: Timestamp,
) -> Result<Preview, StoreError> {
    let settings = queries::settings(db)?;
    let account = settings.account();
    let mut proposed = account.clone();
    let effective_date = simulate(&mut change, &settings, &mut proposed)?;
    let mut observations = queries::observations(db, observed_at, None)?;
    observations.today_finished = queries::today_finished(db, today)?;
    Ok(Preview {
        change,
        revision: revision(db)?,
        as_of,
        today,
        observed_at,
        effective_date,
        before: impact(&account, &observations, as_of, today, effective_date)?,
        after: impact(&proposed, &observations, as_of, today, effective_date)?,
    })
}

fn simulate(
    change: &mut Change,
    settings: &Settings,
    proposed: &mut Account,
) -> Result<NaiveDate, StoreError> {
    let date = match change {
        Change::Schedule {
            effective_date,
            schedule,
        } => {
            proposed.schedules.insert(*effective_date, *schedule);
            *effective_date
        }
        Change::Opening(opening) => {
            proposed.opening = Some(*opening);
            opening.effective_date
        }
        Change::SetTarget {
            date,
            seconds,
            reason,
        } => {
            trim_reason(reason)?;
            proposed.overrides.insert(*date, *seconds);
            *date
        }
        Change::ClearTarget { date, reason } => {
            trim_reason(reason)?;
            if proposed.overrides.remove(date).is_none() {
                return Err(StoreError::OverrideUnavailable(*date));
            }
            *date
        }
        Change::AddAdjustment { adjustment, reason } => {
            trim_reason(reason)?;
            proposed.adjustments.push(*adjustment);
            adjustment.effective_date
        }
        Change::EditAdjustment {
            id,
            adjustment,
            reason,
        } => {
            trim_reason(reason)?;
            let index = active_adjustment_index(settings, *id)?;
            proposed.adjustments[index] = *adjustment;
            adjustment.effective_date
        }
        Change::VoidAdjustment { id, reason } => {
            trim_reason(reason)?;
            let index = active_adjustment_index(settings, *id)?;
            proposed.adjustments.remove(index).effective_date
        }
    };
    if !(0..=9999).contains(&date.year()) {
        return Err(AccountError::InvalidDateRange.into());
    }
    Ok(date)
}

fn active_adjustment_index(settings: &Settings, id: i64) -> Result<usize, StoreError> {
    settings
        .adjustments
        .iter()
        .filter(|item| !item.voided)
        .position(|item| item.id == id)
        .ok_or(StoreError::AdjustmentUnavailable(id))
}

fn trim_reason(reason: &mut String) -> Result<(), StoreError> {
    let trimmed = reason.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err(StoreError::ReasonRequired);
    }
    if trimmed.len() != reason.len() {
        *reason = trimmed.to_owned();
    }
    Ok(())
}

fn impact(
    account: &Account,
    observations: &Observations,
    as_of: NaiveDate,
    today: NaiveDate,
    effective_date: NaiveDate,
) -> Result<Impact, StoreError> {
    let balance = match account.finalized_balance_as_of(as_of, today, observations) {
        Ok(Some(value)) => BalanceState::Calculated(value),
        Ok(None) => BalanceState::BeforeOpening,
        Err(AccountError::MissingOpening) => BalanceState::Unconfigured,
        Err(AccountError::MissingSchedule(date)) => BalanceState::MissingSchedule(date),
        Err(error) => return Err(error.into()),
    };
    Ok(Impact {
        balance,
        target_at_effective_date: account.target_on(effective_date),
        provisional: matches!(balance, BalanceState::Calculated(_))
            && observations
                .uncertain_since
                .is_some_and(|date| date <= as_of),
    })
}

const SCHEDULE_JSON: &str =
    "SELECT json_object('id', id, 'effective_date', effective_date, 'weekly_seconds', weekly_seconds,
                        'weekdays', json_array(monday, tuesday, wednesday, thursday, friday, saturday, sunday),
                        'created_at_utc_ms', created_at_utc_ms)
     FROM work_schedules WHERE effective_date = ?1 AND superseded_at_utc_ms IS NULL";
const OPENING_JSON: &str =
    "SELECT json_object('id', id, 'effective_date', effective_date, 'signed_seconds', signed_seconds,
                        'created_at_utc_ms', created_at_utc_ms)
     FROM opening_balance_versions WHERE superseded_at_utc_ms IS NULL";
const OVERRIDE_JSON: &str =
    "SELECT json_object('date', effective_date, 'seconds', target_seconds, 'reason', reason,
                        'created_at_utc_ms', created_at_utc_ms, 'updated_at_utc_ms', updated_at_utc_ms)
     FROM daily_target_overrides WHERE effective_date = ?1";
const ADJUSTMENT_JSON: &str =
    "SELECT json_object('id', id, 'effective_date', effective_date, 'signed_seconds', signed_seconds,
                        'reason', reason, 'voided_at_utc_ms', voided_at_utc_ms,
                        'created_at_utc_ms', created_at_utc_ms, 'updated_at_utc_ms', updated_at_utc_ms)
     FROM balance_adjustments WHERE id = ?1";

pub(super) fn write(db: &Connection, change: Change, now: i64) -> Result<(), StoreError> {
    let (entity, key, action, reason, before, after) = match change {
        Change::Schedule {
            effective_date,
            schedule,
        } => {
            let key = effective_date.to_string();
            let before = json(db, SCHEDULE_JSON, [&key])?;
            db.execute(
                "UPDATE work_schedules SET superseded_at_utc_ms = ?1
                 WHERE effective_date = ?2 AND superseded_at_utc_ms IS NULL",
                params![now, key],
            )?;
            let [mon, tue, wed, thu, fri, sat, sun] = schedule.weekdays();
            db.execute(
                "INSERT INTO work_schedules
                 (effective_date, weekly_seconds, monday, tuesday, wednesday, thursday, friday, saturday, sunday, created_at_utc_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![key, schedule.weekly_seconds(), mon, tue, wed, thu, fri, sat, sun, now],
            )?;
            let after = json(db, SCHEDULE_JSON, [&key])?;
            ("schedule", key, upsert_action(&before), None, before, after)
        }
        Change::Opening(opening) => {
            let before = json(db, OPENING_JSON, [])?;
            db.execute(
                "UPDATE opening_balance_versions SET superseded_at_utc_ms = ?1 WHERE superseded_at_utc_ms IS NULL",
                [now],
            )?;
            db.execute(
                "INSERT INTO opening_balance_versions (effective_date, signed_seconds, created_at_utc_ms)
                 VALUES (?1, ?2, ?3)",
                params![opening.effective_date.to_string(), opening.signed_seconds, now],
            )?;
            let after = json(db, OPENING_JSON, [])?;
            (
                "opening",
                "opening".into(),
                upsert_action(&before),
                None,
                before,
                after,
            )
        }
        Change::SetTarget {
            date,
            seconds,
            reason,
        } => {
            let key = date.to_string();
            let before = json(db, OVERRIDE_JSON, [&key])?;
            db.execute(
                "INSERT INTO daily_target_overrides
                 (effective_date, target_seconds, reason, created_at_utc_ms, updated_at_utc_ms)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(effective_date) DO UPDATE SET target_seconds = excluded.target_seconds,
                   reason = excluded.reason, updated_at_utc_ms = excluded.updated_at_utc_ms",
                params![key, seconds, reason, now],
            )?;
            let after = json(db, OVERRIDE_JSON, [&key])?;
            (
                "target",
                key,
                upsert_action(&before),
                Some(reason),
                before,
                after,
            )
        }
        Change::ClearTarget { date, reason } => {
            let key = date.to_string();
            let before = json(db, OVERRIDE_JSON, [&key])?;
            db.execute(
                "DELETE FROM daily_target_overrides WHERE effective_date = ?1",
                [&key],
            )?;
            ("target", key, "clear", Some(reason), before, None)
        }
        Change::AddAdjustment { adjustment, reason } => {
            db.execute(
                "INSERT INTO balance_adjustments
                 (effective_date, signed_seconds, reason, created_at_utc_ms, updated_at_utc_ms)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![
                    adjustment.effective_date.to_string(),
                    adjustment.signed_seconds,
                    reason,
                    now
                ],
            )?;
            let id = db.last_insert_rowid();
            (
                "adjustment",
                id.to_string(),
                "add",
                Some(reason),
                None,
                json(db, ADJUSTMENT_JSON, [id])?,
            )
        }
        Change::EditAdjustment {
            id,
            adjustment,
            reason,
        } => {
            let before = json(db, ADJUSTMENT_JSON, [id])?;
            db.execute(
                "UPDATE balance_adjustments SET effective_date = ?1, signed_seconds = ?2,
                   reason = ?3, updated_at_utc_ms = ?4 WHERE id = ?5",
                params![
                    adjustment.effective_date.to_string(),
                    adjustment.signed_seconds,
                    reason,
                    now,
                    id
                ],
            )?;
            (
                "adjustment",
                id.to_string(),
                "edit",
                Some(reason),
                before,
                json(db, ADJUSTMENT_JSON, [id])?,
            )
        }
        Change::VoidAdjustment { id, reason } => {
            let before = json(db, ADJUSTMENT_JSON, [id])?;
            db.execute(
                "UPDATE balance_adjustments SET voided_at_utc_ms = ?1, updated_at_utc_ms = ?1 WHERE id = ?2",
                params![now, id],
            )?;
            (
                "adjustment",
                id.to_string(),
                "void",
                Some(reason),
                before,
                json(db, ADJUSTMENT_JSON, [id])?,
            )
        }
    };
    db.execute(
        "INSERT INTO account_audit_events
         (entity_type, entity_key, action, reason, old_value_json, new_value_json, occurred_at_utc_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![entity, key, action, reason, before, after, now],
    )?;
    Ok(())
}

fn json(
    db: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> rusqlite::Result<Option<String>> {
    db.query_row(sql, params, |row| row.get(0)).optional()
}

fn upsert_action(before: &Option<String>) -> &'static str {
    if before.is_some() { "edit" } else { "add" }
}
