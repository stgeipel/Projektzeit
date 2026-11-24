slint::slint! {
    #[edition = "1.19"]
    import { AboutSlint, CheckBox, ComboBox, LineEdit, Palette, ScrollView } from "std-widgets.slint";

    export struct ScheduleInputs { values: [string], message: string, invalid: bool, date: string }
    export struct MiniOptions { auto-show: bool, topmost: bool, remember: bool }
    export struct Metric { label: string, value: string }
    export struct DashboardBalance { due: string, actual: string, delta: string, period: string }
    export struct DayRow { date: string, actual: string, target: string, delta: string }
    export struct SegmentRow {
        title: string, detail: string, time: string, begin: string, finish: string,
        amount: string, pending: bool, pause: bool, id: string, day: string, editable: bool, swatch: color,
    }
    export struct AdjustmentRow { id: string, date: string, reason: string, amount: string, voided: bool }
    export struct TrackingView {
        mode: int, selection: int, projects: [string], worked: string, remaining: string,
        target: string, breaks: string, finish: string, clock: string, progress: float, project: string,
        segment-time: string, delta: string, pending-id: string, pending-count: int, can-finalize: bool, swatch: color,
    }
    export struct EntryForm {
        kind: int, bounds: bool, existing: bool, project: string, activity: string,
        start-date: string, end-date: string, start-time: string, end-time: string,
    }
    export struct ProjectForm { id: string, name: string, description: string, color: string, archived: bool, swatch: color }
    export struct ProjectChoice { id: string }
    export struct EditFields {
        kind: int, date: string, amount: string,
        monday: string, tuesday: string, wednesday: string, thursday: string,
        friday: string, saturday: string, sunday: string,
        reason: string, entry-id: string,
    }

    global Theme {
        private property <color> system-background: Palette.background;
        out property <bool> dark: system-background.red < 128;
        out property <color> canvas: dark ? #13251f : #f1f7f7;
        out property <color> surface: dark ? #1b2f28 : #ffffff;
        out property <color> tint: dark ? #253f33 : #e4f6ef;
        out property <color> border: dark ? #3b5145 : #d4e5e2;
        out property <color> ink: dark ? #f2f5ed : #0f172a;
        out property <color> muted: dark ? #b2c4b8 : #46576a;
        out property <color> green: dark ? #a0edc2 : #0e5a47;
        out property <color> focus: dark ? #b8e9bf : #0e5a47;
        out property <color> warning: dark ? #f2cd91 : #764718;
        out property <color> rail: #07382d;
        out property <color> rail-active: #12865d;
        out property <color> rail-accent: #44dca8;
        out property <color> hero-green: #075540;
        out property <color> raised: dark ? #243b32 : #f9fcfc;
        out property <color> danger: dark ? #ffb4ad : #ad2621;
        out property <color> rail-ink: #f3f9ed;
        out property <color> rail-muted: #bad1c0;
        out property <color> chart: dark ? #34d399 : #0B9b57;
        out property <color> track: dark ? #345747 : #dbe7ee;
        out property <color> pause-tint: dark ? #43392a : #fff5dc;
        out property <color> on-action: dark ? #15231d : #ffffff;
    }

    component Label inherits Text {
        color: Theme.muted;
        font-size: 14px;
    }

    component SectionTitle inherits Text {
        color: Theme.ink;
        font-size: 16px;
        font-weight: 680;
    }

    component Card inherits Rectangle {
        background: Theme.surface;
        border-color: Theme.border;
        border-width: 1px;
        border-radius: 12px;
    }

    component LineIcon inherits Path {
        in property <string> shape;
        in property <color> ink: Theme.green;
        commands: root.shape;
        fill: transparent;
        stroke: root.ink;
        stroke-width: 1.8px;
        viewbox-width: 24;
        viewbox-height: 24;
    }

    component ActionButton inherits Rectangle {
        in property <string> label;
        in property <bool> primary;
        in property <bool> nav;
        in property <bool> selected;
        in property <bool> align-left;
        in property <bool> enabled: true;
        in property <string> icon;
        in property <string> detail;
        in property <bool> large;
        in property <bool> tile;
        in property <bool> danger;
        in property <bool> icon-only;
        private property <length> icon-size: root.large ? 34px : root.tile ? 25px : 18px;
        private property <length> caption-x: root.icon == "" ? 14px : (root.large || root.tile ? root.icon-size + 31px : root.nav ? 34px : 39px);
        callback clicked;

        min-width: root.icon-only ? 32px
            : (root.tile ? 112px : caption.preferred-width + root.caption-x + 14px);
        preferred-width: root.tile ? caption.preferred-width + root.caption-x + 16px : self.min-width;
        max-width: root.icon-only ? 32px : 10000px;
        height: root.large ? 78px : root.tile ? 64px : root.icon-only ? 34px : 38px;
        horizontal-stretch: root.align-left ? 1 : 0;
        border-radius: root.large || root.tile ? 12px : 8px;
        border-width: focus.has-focus ? 2px : 1px;
        border-color: focus.has-focus ? (root.nav ? Theme.rail-ink : Theme.focus)
            : root.nav || root.primary || root.icon-only ? transparent : Theme.border;
        background: !root.enabled ? Theme.tint
            : root.nav ? (root.selected ? Theme.rail-accent : touch.has-hover ? Theme.rail-active : transparent)
            : root.large ? (touch.has-hover ? #096f50 : Theme.hero-green)
            : root.primary ? (touch.has-hover ? Theme.green.darker(0.12) : Theme.green)
            : root.tile ? (touch.has-hover ? Theme.tint : Theme.raised)
            : root.icon-only ? (touch.has-hover ? Theme.tint : transparent)
            : touch.has-hover || root.selected ? Theme.tint : Theme.surface;
        forward-focus: focus;

        accessible-role: button;
        accessible-label: root.label;
        accessible-enabled: root.enabled;
        accessible-checkable: root.nav;
        accessible-checked: root.selected;
        accessible-action-default => { if (root.enabled) { root.clicked(); } }

        if root.icon != "": LineIcon {
            x: root.icon-only ? (parent.width - self.width) / 2 : (root.large || root.tile ? 16px : 13px);
            y: (parent.height - self.height) / 2;
            width: root.icon-size; height: root.icon-size;
            shape: root.icon;
            ink: root.danger ? Theme.danger : (root.nav && root.selected ? Theme.hero-green : root.nav || root.large ? Theme.rail-ink : Theme.green);
        }
        caption := Text {
            x: root.caption-x; y: root.detail == "" ? 0px : 9px;
            width: max(0px, parent.width - self.x - 12px);
            height: root.detail == "" ? parent.height : (root.large ? 35px : 22px);
            visible: !root.icon-only;
            text: root.label;
            overflow: elide;
            color: !root.enabled ? Theme.muted : root.nav ? (root.selected ? Theme.hero-green : Theme.rail-ink)
                : root.large ? Theme.rail-ink : root.primary ? Theme.on-action : Theme.ink;
            font-size: root.large ? 27px : root.tile ? 16px : root.nav ? 13px : 14px;
            font-weight: root.selected || root.primary || root.tile ? 690 : 520;
            vertical-alignment: center;
            horizontal-alignment: root.align-left ? left : center;
        }
        if root.detail != "": Label {
            x: root.caption-x;
            y: root.large ? 46px : 33px;
            width: max(0px, parent.width - self.x - 12px);
            text: root.detail;
            font-size: root.large ? 14px : 12px;
            color: root.large ? Theme.rail-ink : Theme.muted;
            overflow: elide;
        }
        touch := TouchArea {
            enabled: root.enabled;
            clicked => { focus.focus(); root.clicked(); }
        }
        focus := FocusScope {
            width: 0px;
            enabled: root.enabled;
            key-pressed(event) => {
                if (event.text == Key.Space || event.text == Key.Return) {
                    root.clicked();
                    return accept;
                }
                return reject;
            }
        }
    }

    component BalancePanel inherits Card {
        in property <[Metric]> metrics;
        in property <string> heading;
        VerticalLayout {
            padding: 18px;
            spacing: 10px;
            alignment: start;
            SectionTitle { text: root.heading; }
            for metric in root.metrics: HorizontalLayout {
                spacing: 8px;
                Label { text: metric.label; wrap: word-wrap; horizontal-stretch: 1; }
                Text { text: metric.value; color: Theme.ink; font-weight: 660; horizontal-stretch: 0; }
            }
        }
    }

    component DayLine inherits Rectangle {
        in property <DayRow> day;
        in property <bool> header;
        height: 34px;
        background: root.header ? Theme.tint : Theme.surface;
        border-radius: root.header ? 5px : 0px;
        private property <length> column-width: (self.width - 20px) / 4;
        Text { x: 10px; width: root.column-width; height: parent.height; text: root.day.date; color: Theme.ink; font-size: 14px; vertical-alignment: center; }
        Text { x: 10px + root.column-width; width: root.column-width; height: parent.height; text: root.day.actual; color: Theme.ink; font-size: 14px; horizontal-alignment: right; vertical-alignment: center; }
        Text { x: 10px + root.column-width * 2; width: root.column-width; height: parent.height; text: root.day.target; color: Theme.ink; font-size: 14px; horizontal-alignment: right; vertical-alignment: center; }
        Text { x: 10px + root.column-width * 3; width: root.column-width; height: parent.height; text: root.day.delta; color: Theme.ink; font-size: 14px; horizontal-alignment: right; vertical-alignment: center; }
        if !root.header: Rectangle { y: parent.height - 1px; height: 1px; background: Theme.border; }
    }

    component PageNav inherits Rectangle {
        in property <int> page;
        in property <bool> compact;
        callback navigate(int);
        property <[string]> links: ["Heute", "Zeiten", "Arbeitszeitkonto", "Einstellungen"];
        property <[string]> icons: [
            "M3 11 L12 3 L21 11 M5 10 V21 H10 V15 H14 V21 H19 V10",
            "M12 3 A9 9 0 1 0 12 21 A9 9 0 1 0 12 3 M10 8 L16 12 L10 16 Z",
            "M4 20 V13 H7 V20 Z M10 20 V9 H13 V20 Z M16 20 V4 H19 V20 Z",
            "M12 3 L14 6 L17 5 L19 7 L18 10 L21 12 L18 14 L19 17 L17 19 L14 18 L12 21 L10 18 L7 19 L5 17 L6 14 L3 12 L6 10 L5 7 L7 5 L10 6 Z M12 9 A3 3 0 1 0 12 15 A3 3 0 1 0 12 9",
        ];
        background: Theme.rail;

        if !root.compact: VerticalLayout {
            padding: 8px;
            spacing: 6px;
            for link[index] in root.links: ActionButton {
                label: link; nav: true; align-left: true; selected: index == root.page; icon: root.icons[index];
                clicked => { root.navigate(index); }
            }
            Rectangle { vertical-stretch: 1; }
        }
        if root.compact: HorizontalLayout {
            padding: 8px;
            spacing: 3px;
            for link[index] in root.links: ActionButton {
                label: link; nav: true; selected: index == root.page;
                horizontal-stretch: 1;
                clicked => { root.navigate(index); }
            }
        }
    }

    component WeekdayField inherits VerticalLayout {
        in property <string> label;
        in property <bool> editable: true;
        in property <string> initial: "00:00";
        out property <string> amount: field.text;
        callback edit;
        spacing: 4px;
        Label { text: root.label; }
        field := LineEdit {
            text: root.initial; enabled: root.editable; accessible-label: root.label;
            edited => { root.edit(); }
            min-width: 50px; preferred-width: 72px; horizontal-stretch: 1;
        }
    }

    component CurrentSchedule inherits Card {
        in property <ScheduleInputs> draft;
        callback edit(int, string);
        VerticalLayout {
            padding: 16px;
            spacing: 8px;
            SectionTitle { text: "Aktuelle Arbeitszeit"; }
            Label {
                text: "Änderungen gelten ab " + root.draft.date
                    + ". Das Wochensoll verteilt sich auf die bestehenden Arbeitstage. Tagesänderungen aktualisieren die Wochensumme.";
                wrap: word-wrap;
            }
            HorizontalLayout {
                spacing: 8px;
                Label { text: "Wochensoll"; vertical-alignment: center; horizontal-stretch: 0; }
                LineEdit {
                    accessible-label: "Wochensoll";
                    text: root.draft.values[0];
                    placeholder-text: "z. B. 38:00";
                    horizontal-stretch: 0;
                    edited(value) => { root.edit(0, value); }
                }
                Rectangle { horizontal-stretch: 1; }
            }
            HorizontalLayout {
                spacing: 5px;
                for day[index] in ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]: VerticalLayout {
                    spacing: 4px;
                    Label { text: day; }
                    LineEdit {
                        min-width: 50px;
                        preferred-width: 72px;
                        text: root.draft.values[index + 1];
                        accessible-label: "Tagessoll " + day;
                        edited(value) => { root.edit(index + 1, value); }
                    }
                }
            }
            if root.draft.message != "": Label {
                text: root.draft.message;
                color: root.draft.invalid ? Theme.warning : Theme.green;
                wrap: word-wrap;
            }
        }
    }

    component MiniSettings inherits Card {
        in property <MiniOptions> mini-options;
        callback change-option(int, bool);
        callback reset-position;
        VerticalLayout {
            padding: 16px;
            spacing: 8px;
            SectionTitle { text: "Mini-Timer"; }
            CheckBox {
                text: "Mini-Timer beim Minimieren anzeigen";
                checked: root.mini-options.auto-show;
                toggled => { root.change-option(0, self.checked); }
            }
            CheckBox {
                text: "Mini-Timer immer im Vordergrund";
                checked: root.mini-options.topmost;
                toggled => { root.change-option(1, self.checked); }
            }
            CheckBox {
                text: "Letzte Fensterposition merken";
                checked: root.mini-options.remember;
                toggled => { root.change-option(2, self.checked); }
            }
            HorizontalLayout {
                ActionButton { label: "Fensterposition zurücksetzen"; clicked => { root.reset-position(); } }
                Rectangle { horizontal-stretch: 1; }
            }
        }
    }

    component AccountSettings inherits VerticalLayout {
        in property <int> scheme;
        in property <ScheduleInputs> schedule;
        in property <EditFields> dated-draft;
        in property <MiniOptions> mini-options;
        in property <bool> corrections;
        in property <string> opening-summary;
        in property <[AdjustmentRow]> adjustments;
        in property <string> date;
        in property <[string]> settings;
        in property <[string]> history;
        in property <string> preview-text;
        in property <bool> preview-ready;
        callback change-scheme(int);
        callback edit-schedule(int, string);
        callback change-mini-option(int, bool);
        callback reset-mini-position;
        callback preview-edit(EditFields);
        callback capture-dated(EditFields);
        callback confirm-edit();
        callback cancel-edit();
        function fields() -> EditFields {
            return {
                kind: kind.operation, date: date-field.text, amount: amount.text,
                monday: monday.amount, tuesday: tuesday.amount, wednesday: wednesday.amount,
                thursday: thursday.amount, friday: friday.amount, saturday: saturday.amount,
                sunday: sunday.amount, reason: reason.text, entry-id: entry.text
            };
        }
        function remember-dated() {
            if (!root.corrections) { root.capture-dated(root.fields()); }
        }
        spacing: 14px;
        alignment: start;

        if !root.corrections: Card {
            HorizontalLayout {
                padding: 16px;
                spacing: 12px;
                SectionTitle { text: "Erscheinungsbild"; vertical-alignment: center; }
                Rectangle { horizontal-stretch: 1; }
                appearance := ComboBox {
                    model: ["System", "Hell", "Dunkel"];
                    current-index: root.scheme;
                    accessible-label: "Erscheinungsbild";
                    min-width: 140px;
                    selected => { root.change-scheme(appearance.current-index); }
                }
            }
        }

        if !root.corrections: CurrentSchedule {
            draft: root.schedule;
            edit(index, value) => { root.edit-schedule(index, value); }
        }
        if !root.corrections: MiniSettings {
            mini-options: root.mini-options;
            change-option(index, value) => { root.change-mini-option(index, value); }
            reset-position => { root.reset-mini-position(); }
        }
        Card {
            VerticalLayout {
                padding: 16px;
                spacing: 10px;
                SectionTitle { text: root.corrections ? "Startsaldo und Saldoanpassungen" : "Datierte Arbeitszeitregeln"; }
                if root.corrections: Label { text: root.opening-summary; }
                kind := ComboBox {
                    property <int> operation: root.corrections ? (self.current-index == 0 ? 1 : self.current-index + 3)
                        : [0, 2, 3][self.current-index];
                    model: root.corrections
                        ? ["Startsaldo", "Neue Korrektur", "Korrektur bearbeiten", "Korrektur stornieren"]
                        : ["Wochenplan mit Gültigkeitsdatum", "Tagessoll setzen", "Tagessoll zurücksetzen"];
                    current-index: root.corrections ? 0 : (root.dated-draft.kind == 0 ? 0 : root.dated-draft.kind - 1);
                    accessible-label: "Art der Änderung";
                    selected => { root.remember-dated(); }
                    enabled: !root.preview-ready;
                }
                HorizontalLayout {
                    spacing: 8px;
                    date-field := LineEdit {
                        enabled: !root.preview-ready && kind.operation != 6;
                        placeholder-text: "Gültig ab (TT.MM.JJJJ)";
                        accessible-label: "Gültig ab";
                        text: root.corrections ? root.date : root.dated-draft.date;
                        edited => { root.remember-dated(); }
                        min-width: 105px;
                    }
                    amount := LineEdit {
                        enabled: !root.preview-ready && kind.operation != 3 && kind.operation != 6;
                        placeholder-text: kind.operation == 0 ? "Wochensoll (hh:mm)" : "Dauer (±hh:mm)";
                        accessible-label: "Dauer";
                        text: root.corrections ? "" : root.dated-draft.amount;
                        edited => { root.remember-dated(); }
                        min-width: 110px;
                    }
                }
                HorizontalLayout {
                    visible: kind.operation == 0;
                    height: kind.operation == 0 ? self.preferred-height : 0px;
                    spacing: 5px;
                    monday := WeekdayField {
                        editable: !root.preview-ready; label: "Mo";
                        initial: root.corrections ? "00:00" : root.dated-draft.monday;
                        edit => { root.remember-dated(); }
                    }
                    tuesday := WeekdayField {
                        editable: !root.preview-ready; label: "Di";
                        initial: root.corrections ? "00:00" : root.dated-draft.tuesday;
                        edit => { root.remember-dated(); }
                    }
                    wednesday := WeekdayField {
                        editable: !root.preview-ready; label: "Mi";
                        initial: root.corrections ? "00:00" : root.dated-draft.wednesday;
                        edit => { root.remember-dated(); }
                    }
                    thursday := WeekdayField {
                        editable: !root.preview-ready; label: "Do";
                        initial: root.corrections ? "00:00" : root.dated-draft.thursday;
                        edit => { root.remember-dated(); }
                    }
                    friday := WeekdayField {
                        editable: !root.preview-ready; label: "Fr";
                        initial: root.corrections ? "00:00" : root.dated-draft.friday;
                        edit => { root.remember-dated(); }
                    }
                    saturday := WeekdayField {
                        editable: !root.preview-ready; label: "Sa";
                        initial: root.corrections ? "00:00" : root.dated-draft.saturday;
                        edit => { root.remember-dated(); }
                    }
                    sunday := WeekdayField {
                        editable: !root.preview-ready; label: "So";
                        initial: root.corrections ? "00:00" : root.dated-draft.sunday;
                        edit => { root.remember-dated(); }
                    }
                }
                HorizontalLayout {
                    spacing: 8px;
                    reason := LineEdit {
                        enabled: !root.preview-ready && kind.operation >= 2;
                        placeholder-text: "Begründung";
                        accessible-label: "Begründung";
                        text: root.corrections ? "" : root.dated-draft.reason;
                        edited => { root.remember-dated(); }
                        min-width: 90px;
                    }
                    entry := LineEdit {
                        enabled: !root.preview-ready && kind.operation >= 5;
                        placeholder-text: "Korrektur-Nr.";
                        accessible-label: "Korrektur-Nr.";
                        min-width: 90px;
                    }
                    ActionButton {
                        label: "Vorschau"; primary: true; enabled: !root.preview-ready;
                        clicked => { root.preview-edit(root.fields()); }
                    }
                }
                if root.preview-ready: Card {
                    background: Theme.tint;
                    VerticalLayout {
                        padding: 12px;
                        spacing: 10px;
                        Label { text: root.preview-text; color: Theme.ink; wrap: word-wrap; }
                        HorizontalLayout {
                            spacing: 8px;
                            ActionButton { label: "Abbrechen"; clicked => { root.cancel-edit(); } }
                            ActionButton { label: "Speichern"; primary: true; clicked => { root.confirm-edit(); } }
                        }
                    }
                }
            }
        }

        if root.corrections: Card {
            VerticalLayout {
                padding: 16px;
                spacing: 8px;
                SectionTitle { text: "Gespeicherte Korrekturen"; }
                if root.adjustments.length == 0: Label { text: "Noch keine Korrekturen erfasst."; }
                for adjustment in root.adjustments: HorizontalLayout {
                    spacing: 12px;
                    Label { text: "#" + adjustment.id; }
                    Label { text: adjustment.date; }
                    Label { text: adjustment.reason; wrap: word-wrap; horizontal-stretch: 1; }
                    Label { text: adjustment.voided ? "Storniert" : adjustment.amount; }
                }
            }
        }
        if !root.corrections: Card {
            VerticalLayout {
                padding: 16px;
                spacing: 9px;
                SectionTitle { text: "Gespeicherte Einstellungen"; }
                for item in root.settings: Label { text: item; wrap: word-wrap; }
            }
        }
        Card {
            VerticalLayout {
                padding: 16px;
                spacing: 9px;
                SectionTitle { text: "Änderungsverlauf"; }
                for item in root.history: Label { text: item; wrap: word-wrap; }
            }
        }
    }

    component ProgressTrack inherits Rectangle {
        in property <float> progress;
        height: 10px;
        border-radius: 5px;
        background: Theme.tint;
        Rectangle {
            x: 0;
            width: parent.width * root.progress;
            height: parent.height;
            border-radius: 5px;
            background: Theme.chart;
        }
    }

    component ProjectSelector inherits ComboBox {
        in property <int> selection;
        current-index: root.selection;
        changed selection => { root.current-index = root.selection; }
    }

    component ActivityPrompt inherits VerticalLayout {
        in property <string> pending-id;
        callback save(string);
        callback defer;
        spacing: 6px;
        changed pending-id => { activity.text = ""; }
        Label { text: "Tätigkeit für den beendeten Abschnitt"; color: Theme.ink; }
        activity := LineEdit {
            placeholder-text: "Tätigkeit beschreiben"; accessible-label: "Tätigkeitsbeschreibung";
            accepted => { root.save(self.text); }
        }
        HorizontalLayout {
            spacing: 6px;
            ActionButton { label: "Speichern"; primary: true; clicked => { root.save(activity.text); } }
            ActionButton { label: "Später"; clicked => { root.defer(); } }
        }
    }

    component ProjectPicker inherits HorizontalLayout {
        in property <TrackingView> data;
        in property <bool> featured;
        callback select(int);
        callback add;
        spacing: 8px;
        if root.featured: LineIcon {
            width: 27px; height: 27px;
            shape: "M3 6 H10 L12 8 H21 V19 H3 Z";
        }
        VerticalLayout {
            horizontal-stretch: 1;
            spacing: 4px;
            if root.featured: Label { text: "Projekt wählen"; color: Theme.ink; font-weight: 640; }
            selector := ProjectSelector {
                model: root.data.projects; selection: root.data.selection;
                accessible-label: "Projekt wählen"; horizontal-stretch: 1;
                selected => { root.select(self.current-index); }
            }
        }
        ActionButton {
            label: "Projekt direkt anlegen"; icon-only: true;
            icon: "M12 5 V19 M5 12 H19";
            clicked => { root.add(); }
        }
    }

    component TrackingControls inherits Card {
        in property <TrackingView> data;
        in property <bool> compact;
        in property <bool> featured;
        callback command(int);
        callback select-project(int);
        callback create-project(string);
        callback describe(string);
        private property <bool> new-project;
        private property <int> mode: root.data.mode;
        background: root.featured ? Theme.surface : Theme.tint;
        horizontal-stretch: 1;
        VerticalLayout {
            padding: root.compact ? 12px : 14px;
            spacing: 9px;
            if !root.featured || root.mode == 1 || root.mode == 2 || root.mode == 4:
                SectionTitle { text: ["Bereit für den Arbeitstag", "Arbeit läuft", "Pause läuft", "Arbeitstag beendet", "Offene Erfassung wiederaufnehmen"][root.mode]; }
            if root.mode == 1: HorizontalLayout {
                spacing: 8px;
                Rectangle { width: 9px; height: 9px; border-radius: 5px; background: root.data.swatch; }
                Label {
                    text: root.data.project + " · " + root.data.segment-time;
                    color: Theme.green; horizontal-stretch: 1; overflow: elide;
                }
            }
            if (root.mode == 0 || root.mode == 3) && root.featured: HorizontalLayout {
                spacing: 16px;
                ActionButton {
                    label: root.mode == 3 ? "Weiterarbeiten" : "Arbeit starten";
                    detail: root.mode == 3 ? "Arbeitstag beendet" : "Arbeitszeit erfassen";
                    primary: true; large: true; align-left: true;
                    icon: "M12 2 A10 10 0 1 0 12 22 A10 10 0 1 0 12 2 M10 7 L17 12 L10 17 Z";
                    clicked => { root.command(0); }
                }
                Rectangle { width: 1px; background: Theme.border; }
                ProjectPicker {
                    featured: true; data: root.data;
                    horizontal-stretch: 1;
                    select(index) => { root.select-project(index); }
                    add => { root.new-project = !root.new-project; }
                }
            }
            if (root.mode == 0 || root.mode == 3) && !root.featured: ActionButton {
                label: root.mode == 3 ? "Weiterarbeiten" : "Arbeit starten"; primary: true;
                clicked => { root.command(0); }
            }
            if root.mode < 4 && (!root.featured || (root.mode == 1 || root.mode == 2)): ProjectPicker {
                data: root.data;
                select(index) => { root.select-project(index); }
                add => { root.new-project = !root.new-project; }
            }
            if root.data.can-finalize: ActionButton { label: "Tag abschließen"; clicked => { root.command(10); } }
            if root.new-project && root.mode < 4: HorizontalLayout {
                spacing: 6px;
                project-name := LineEdit {
                    placeholder-text: "Projektname"; accessible-label: "Projektname";
                    horizontal-stretch: 1;
                }
                ActionButton { label: "Anlegen"; enabled: project-name.text != ""; clicked => { root.create-project(project-name.text); root.new-project = false; } }
            }
            if root.mode == 1 || root.mode == 2: HorizontalLayout {
                spacing: 5px;
                ActionButton {
                    label: root.mode == 1 ? "Projekt wechseln" : "Weiterarbeiten";
                    primary: true;
                    clicked => { root.command(root.mode == 1 ? 1 : 3); }
                }
                if root.mode == 1: ActionButton { label: "Pause"; clicked => { root.command(2); } }
                ActionButton { label: "Feierabend"; clicked => { root.command(4); } }
            }
            if root.mode == 4: VerticalLayout {
                spacing: 5px;
                Label { text: "Wie soll die Zeit seit dem letzten Speicherpunkt zählen?"; wrap: word-wrap; }
                ActionButton { label: "Laufende Erfassung fortsetzen"; primary: true; clicked => { root.command(5); } }
                ActionButton { label: "Als Pause verbuchen"; clicked => { root.command(6); } }
                ActionButton { label: "Am Speicherpunkt beenden"; clicked => { root.command(7); } }
            }
            if root.data.pending-id != "": ActivityPrompt {
                pending-id: root.data.pending-id;
                save(text) => { root.describe(text); }
                defer => { root.command(8); }
            }
            if root.data.pending-id == "" && root.data.pending-count > 0: ActionButton {
                label: "Fehlende Tätigkeiten (" + root.data.pending-count + ")";
                clicked => { root.command(9); }
            }
        }
    }

    component DayStatus inherits Card {
        in property <TrackingView> data;
        in property <bool> compact;
        VerticalLayout {
            padding: root.compact ? 12px : 18px;
            spacing: 12px;
            HorizontalLayout {
                spacing: root.compact ? 8px : 20px;
                for metric in [
                    { label: "Heute netto", value: root.data.worked },
                    { label: root.data.mode == 3 ? "Tagessaldo" : "Noch zu arbeiten",
                      value: root.data.mode == 3 ? root.data.delta : root.data.remaining },
                    { label: root.data.mode == 2 ? "Laufende Pause" : "Feierabend ca.",
                      value: root.data.mode == 2 ? root.data.segment-time : root.data.finish },
                ]: VerticalLayout {
                    spacing: 3px;
                    horizontal-stretch: 1;
                    Label { text: metric.label; font-size: 12px; }
                    Text { text: metric.value; color: Theme.ink; font-size: root.compact ? 18px : 22px; font-weight: 710; }
                }
            }
            ProgressTrack { progress: root.data.progress; }
            Label { text: "Tagessoll " + root.data.target + " · Pausen " + root.data.breaks; font-size: 12px; }
        }
    }

    component RecordedTimes inherits Card {
        in property <[SegmentRow]> segments;
        in property <bool> history;
        callback edit(string, bool);
        VerticalLayout {
            padding: 14px;
            spacing: 6px;
            SectionTitle { text: root.history ? "Zeitbuchungen" : "Heutige Zeiten"; }
            if root.segments.length == 0: Label {
                text: "Für diesen Zeitraum sind keine Zeiten erfasst.";
                wrap: word-wrap;
            }
            for segment in root.segments: Rectangle {
                background: segment.pause ? Theme.pause-tint : Theme.canvas;
                border-radius: 6px;
                HorizontalLayout {
                    padding: 8px;
                    spacing: 8px;
                    Rectangle { width: 8px; height: 8px; border-radius: 4px; background: segment.swatch; }
                    VerticalLayout {
                        spacing: 3px;
                        horizontal-stretch: 1;
                        Text { text: segment.title; color: Theme.ink; font-weight: 640; wrap: word-wrap; }
                        Label {
                            text: (root.history ? segment.day + " · " : "") + segment.time + " · " + segment.amount;
                            wrap: word-wrap;
                        }
                        Label { text: segment.detail; wrap: word-wrap; color: segment.pending ? Theme.warning : Theme.muted; }
                    }
                    if segment.editable: VerticalLayout {
                        spacing: 3px;
                        ActionButton { label: segment.pending ? "Beschreiben" : "Bearbeiten"; clicked => { root.edit(segment.id, false); } }
                        if root.history: ActionButton { label: "Tagesgrenzen"; clicked => { root.edit(segment.id, true); } }
                    }
                }
            }
        }
    }

    component RecordEditor inherits Card {
        in property <EntryForm> data;
        in property <[ProjectChoice]> choices;
        in property <[string]> names;
        in property <int> selection;
        callback save(EntryForm);
        callback cancel;
        callback remove;
        in property <bool> propose-delete;
        private property <bool> confirm-delete: root.propose-delete;
        changed data => { root.confirm-delete = root.propose-delete; }
        changed propose-delete => { root.confirm-delete = root.propose-delete; }
        background: Theme.tint;
        VerticalLayout {
            padding: 14px;
            spacing: 8px;
            SectionTitle { text: root.data.bounds ? "Arbeitsfenster korrigieren" : (root.data.existing ? "Zeit bearbeiten" : "Zeit hinzufügen"); }
            if root.data.bounds: Label {
                text: "Angrenzende Buchungen werden mitkorrigiert. Pausen und Lücken bleiben erhalten.";
                wrap: word-wrap;
            }
            kind := ComboBox {
                visible: !root.data.bounds;
                model: ["Arbeit", "Pause"]; current-index: root.data.kind;
                accessible-label: "Buchungsart";
            }
            HorizontalLayout {
                spacing: 8px;
                VerticalLayout {
                    spacing: 5px;
                    Label { text: "Beginn"; }
                    start-date := LineEdit { text: root.data.start-date; accessible-label: "Startdatum"; placeholder-text: "TT.MM.JJJJ"; }
                    start-time := LineEdit { text: root.data.start-time; accessible-label: "Beginn"; placeholder-text: "HH:MM"; }
                }
                VerticalLayout {
                    spacing: 5px;
                    Label { text: "Ende"; }
                    end-date := LineEdit { text: root.data.end-date; accessible-label: "Enddatum"; placeholder-text: "TT.MM.JJJJ"; }
                    end-time := LineEdit { text: root.data.end-time; accessible-label: "Ende"; placeholder-text: "HH:MM"; }
                }
            }
            assignment := ComboBox {
                visible: !root.data.bounds && kind.current-index == 0;
                model: root.names;
                current-index: root.selection;
                accessible-label: "Projektzuordnung";
            }
            activity := LineEdit {
                visible: !root.data.bounds && kind.current-index == 0;
                text: root.data.activity;
                placeholder-text: "Tätigkeitsbeschreibung";
                accessible-label: "Beschreibung";
            }
            if root.confirm-delete: Label { text: "Diese Buchung dauerhaft löschen?"; color: Theme.warning; }
            HorizontalLayout {
                spacing: 6px;
                ActionButton { label: "Abbrechen"; clicked => { root.cancel(); } }
                if root.data.existing: ActionButton {
                    label: root.confirm-delete ? "Endgültig löschen" : "Löschen";
                    clicked => {
                        if (root.confirm-delete) { root.remove(); }
                        else { root.confirm-delete = true; }
                    }
                }
                ActionButton {
                    label: "Speichern"; primary: true;
                    clicked => {
                        root.save({
                            kind: kind.current-index, bounds: root.data.bounds, existing: root.data.existing,
                            project: root.choices[assignment.current-index].id, activity: activity.text,
                            start-date: start-date.text, end-date: end-date.text,
                            start-time: start-time.text, end-time: end-time.text,
                        });
                    }
                }
            }
        }
    }

    component ProjectManager inherits Card {
        in property <[ProjectForm]> projects;
        in property <bool> editing;
        in property <ProjectForm> data;
        callback edit(string);
        callback save(ProjectForm);
        callback archive(string);
        callback cancel;
        VerticalLayout {
            padding: 14px;
            spacing: 9px;
            HorizontalLayout {
                spacing: 8px;
                SectionTitle { text: "Projekte"; horizontal-stretch: 1; vertical-alignment: center; }
                ActionButton { label: "+ Neues Projekt"; primary: true; clicked => { root.edit(""); } }
            }
            if root.editing: Card {
                background: Theme.tint;
                VerticalLayout {
                    padding: 12px;
                    spacing: 6px;
                    name := LineEdit { text: root.data.name; placeholder-text: "Projektname"; accessible-label: "Projektname"; }
                    description := LineEdit { text: root.data.description; placeholder-text: "Projektbeschreibung"; accessible-label: "Projektbeschreibung"; }
                    hex := LineEdit { text: root.data.color; placeholder-text: "#RRGGBB"; accessible-label: "Projektfarbe als #RRGGBB"; }
                    HorizontalLayout {
                        spacing: 6px;
                        ActionButton { label: "Abbrechen"; clicked => { root.cancel(); } }
                        ActionButton {
                            label: "Speichern"; primary: true;
                            clicked => { root.save({ id: root.data.id, name: name.text, description: description.text, color: hex.text, archived: root.data.archived, swatch: root.data.swatch }); }
                        }
                    }
                }
            }
            if root.projects.length == 0 && !root.editing: Label { text: "Bislang gibt es keine Projekte."; }
            for project in root.projects: HorizontalLayout {
                spacing: 7px;
                Rectangle { width: 12px; height: 12px; border-radius: 6px; background: project.swatch; }
                VerticalLayout {
                    horizontal-stretch: 1;
                    Label { text: project.name; color: Theme.ink; font-weight: 650; wrap: word-wrap; }
                    Label { text: project.color + (project.archived ? " · Archiviert" : " · Aktiv"); font-size: 12px; }
                    if project.description != "": Label { text: project.description; wrap: word-wrap; }
                }
                ActionButton { label: "Bearbeiten"; clicked => { root.edit(project.id); } }
                ActionButton { label: project.archived ? "Reaktivieren" : "Archivieren"; clicked => { root.archive(project.id); } }
            }
        }
    }

    component GreetingMark inherits Rectangle {
        width: 53px;
        height: 49px;
        LineIcon {
            x: 0px; y: 0px; width: 44px; height: 44px;
            ink: #e8a519;
            shape: "M12 1 V4 M4 4 L6 6 M1 12 H4 M20 4 L18 6 M20 12 H23 M7 13 A5 5 0 0 1 17 13";
        }
        Rectangle { x: 8px; y: 27px; width: 41px; height: 16px; border-radius: 8px; background: #b7ced7; }
        Rectangle { x: 15px; y: 19px; width: 22px; height: 22px; border-radius: 11px; background: #c6d9df; }
        Rectangle { x: 30px; y: 24px; width: 15px; height: 17px; border-radius: 8px; background: #b7ced7; }
    }

    component RunningClock inherits Card {
        in property <TrackingView> data;
        preferred-width: 320px;
        min-width: 212px;
        horizontal-stretch: 1;
        VerticalLayout {
            padding: 12px;
            Rectangle {
                background: Theme.tint;
                border-radius: 12px;
                VerticalLayout {
                    padding: 12px; spacing: 1px;
                    Label {
                        text: root.data.mode == 3 ? "Tageszeit netto" : "Laufende Zeit";
                        color: Theme.ink; font-weight: 620;
                    }
                    Text { text: root.data.clock; color: Theme.green; font-size: 39px; font-weight: 780; }
                }
            }
        }
    }

    component TodayRow inherits Rectangle {
        in property <SegmentRow> entry;
        in property <bool> header;
        callback edit(string);
        callback remove(string);

        background: root.header ? Theme.canvas : (root.entry.pause ? Theme.pause-tint : Theme.surface);
        border-radius: 5px;
        HorizontalLayout {
            padding: 7px; spacing: 6px;
            HorizontalLayout {
                width: 104px; spacing: 5px;
                if !root.header: Rectangle {
                    width: 8px;
                    Rectangle {
                        width: 8px; height: 8px; y: (parent.height - self.height) / 2;
                        border-radius: 4px;
                        background: root.entry.pause ? Theme.warning : root.entry.swatch;
                    }
                }
                Label {
                    text: root.header ? "Projekt" : root.entry.title;
                    color: Theme.ink; font-size: 13px; font-weight: root.header ? 700 : 550;
                    overflow: elide; vertical-alignment: center; horizontal-stretch: 1;
                }
            }
            Label {
                text: root.header ? "Tätigkeit" : root.entry.detail;
                color: root.entry.pending ? Theme.warning : Theme.ink;
                font-size: 13px; font-weight: root.header ? 700 : 480;
                horizontal-stretch: 1; vertical-alignment: center; wrap: word-wrap;
            }
            for field[index] in [
                { label: "Start", value: root.entry.begin },
                { label: "Ende", value: root.entry.finish },
                { label: "Pause", value: root.entry.pause ? root.entry.amount : "–" },
                { label: "Dauer", value: root.entry.pause ? "–" : root.entry.amount },
            ]: Label {
                width: index < 2 ? 42px : 72px;
                text: root.header ? field.label : field.value;
                color: Theme.ink; font-size: 12px; font-weight: root.header ? 700 : 490;
                horizontal-alignment: right; vertical-alignment: center;
            }
            if root.entry.editable && !root.header: HorizontalLayout {
                spacing: 2px;
                ActionButton {
                    label: "Zeit bearbeiten"; icon-only: true;
                    icon: "M4 20 L8 19 L20 7 L17 4 L5 16 Z M14 7 L17 10";
                    clicked => { root.edit(root.entry.id); }
                }
                ActionButton {
                    label: "Zeit löschen"; icon-only: true; danger: true;
                    icon: "M4 6 H20 M9 6 V3 H15 V6 M6 6 L7 21 H17 L18 6 M10 10 V17 M14 10 V17";
                    clicked => { root.remove(root.entry.id); }
                }
            }
            if !root.entry.editable || root.header: Rectangle { width: 66px; height: 28px; }
        }
    }

    component TodayTable inherits Card {
        in property <[SegmentRow]> segments;
        in property <string> date;
        in property <string> total;
        callback edit(string);
        callback remove(string);
        callback browse;
        min-width: 505px;
        horizontal-stretch: 2;
        VerticalLayout {
            padding: 11px;
            spacing: 6px;
            alignment: start;
            HorizontalLayout {
                padding-bottom: 5px; spacing: 8px;
                LineIcon { width: 25px; height: 25px; shape: "M12 3 A9 9 0 1 0 12 21 A9 9 0 1 0 12 3 M12 7 V12 L16 14"; }
                SectionTitle { text: "Heutige Zeiten"; font-size: 20px; vertical-alignment: center; }
                Rectangle { horizontal-stretch: 1; }
                ActionButton {
                    label: root.date; icon: "M4 6 H20 V21 H4 Z M4 10 H20 M8 3 V8 M16 3 V8 M16 14 H17";
                    align-left: true; horizontal-stretch: 0;
                    clicked => { root.browse(); }
                }
            }
            if root.segments.length == 0: Label {
                text: "Heute sind noch keine Zeiten erfasst."; wrap: word-wrap;
            }
            if root.segments.length > 0: TodayRow { header: true; }
            for entry in root.segments: TodayRow {
                entry: entry;
                edit(id) => { root.edit(id); }
                remove(id) => { root.remove(id); }
            }
            Rectangle {
                background: Theme.tint;
                border-radius: 8px;
                HorizontalLayout {
                    padding: 10px; spacing: 8px;
                    SectionTitle { text: "Tagessumme (Netto-Arbeitszeit)"; horizontal-stretch: 1; vertical-alignment: center; }
                    Text { text: root.total; color: Theme.green; font-size: 20px; font-weight: 740; }
                }
            }
        }
    }

    component SnapshotRanges inherits HorizontalLayout {
        in property <int> range;
        callback select(int);
        spacing: 3px;
        horizontal-stretch: 0;
        for caption[index] in ["Heute", "Woche", "Monat"]: ActionButton {
            label: caption; selected: index == root.range; primary: index == root.range;
            clicked => { root.select(index); }
        }
    }

    component AccountSnapshot inherits Card {
        in property <TrackingView> data;
        in property <DashboardBalance> balance;
        in property <[Metric]> summary;
        in property <DashboardBalance> dashboard-balance;
        in property <int> dashboard-range;
        in property <string> dashboard-date;
        in property <bool> confirm-entry-delete;
        in property <int> range;
        in property <bool> inline-ranges;
        callback select(int);
        min-width: 325px;
        preferred-width: 400px;
        horizontal-stretch: 1;
        VerticalLayout {
            padding: 12px; spacing: 9px;
            alignment: start;
            HorizontalLayout {
                spacing: 8px;
                LineIcon { width: 25px; height: 25px; shape: "M12 3 V21 M7 21 H17 M3 7 H21 M6 7 L2 15 H10 Z M18 7 L14 15 H22 Z"; }
                SectionTitle { text: "Arbeitszeitkonto"; font-size: 20px; vertical-alignment: center; horizontal-stretch: 1; }
                if root.inline-ranges: SnapshotRanges {
                    range: root.range; select(index) => { root.select(index); }
                }
            }
            if !root.inline-ranges: HorizontalLayout {
                SnapshotRanges { range: root.range; select(index) => { root.select(index); } }
                Rectangle { horizontal-stretch: 1; }
            }
            if root.range != 0: Label { text: root.balance.period; font-size: 12px; }
            HorizontalLayout {
                spacing: 6px;
                for metric[index] in [
                    { label: root.range == 0 ? "Tagessoll" : "Soll fällig",
                      value: root.range == 0 ? root.data.target : root.balance.due },
                    { label: "Ist netto",
                      value: root.range == 0 ? root.data.worked : root.balance.actual },
                    { label: "Saldo", value: root.range == 0 ? (root.data.mode == 3 ? root.data.delta : "–") : root.balance.delta },
                ]: Rectangle {
                    background: index == 2 ? Theme.tint : Theme.canvas;
                    border-radius: 9px;
                    horizontal-stretch: 1;
                    VerticalLayout {
                        padding: 8px; spacing: 4px;
                        Label { text: metric.label; font-size: 12px; }
                        Text {
                            text: metric.value; color: index == 2 ? Theme.green : Theme.ink;
                            font-size: 17px; font-weight: 740; vertical-alignment: center;
                        }
                    }
                }
            }
            if root.range == 0 && root.data.target != "–": ProgressTrack { progress: root.data.progress; }
            if root.range == 0 && root.data.finish != "–" && (root.data.mode == 1 || root.data.mode == 2): Label {
                text: "Noch " + root.data.remaining + " · Feierabend ca. " + root.data.finish;
                color: Theme.green; wrap: word-wrap;
            }
            if root.range == 0: Label {
                text: root.data.mode == 2 ? "Laufende Pause " + root.data.segment-time : "Pausen heute " + root.data.breaks;
                font-size: 12px;
            }
            if root.range == 0 && root.data.mode != 3 && root.data.target != "–": Label {
                text: "Der heutige Saldo steht nach dem Tagesabschluss fest.";
                font-size: 12px; wrap: word-wrap;
            }
            if root.summary.length == 0: Label {
                text: "Für den Gesamtsaldo braucht das Arbeitszeitkonto einen Wochenplan und einen Startsaldo.";
                font-size: 12px; wrap: word-wrap;
            }
            for item[index] in root.summary: VerticalLayout {
                if index + 1 != root.range: HorizontalLayout {
                    spacing: 7px;
                    Label { text: item.label; font-size: 12px; horizontal-stretch: 1; }
                    Text { text: item.value; color: Theme.ink; font-size: 13px; font-weight: 670; }
                }
            }
        }
    }

    component TodayDashboard inherits VerticalLayout {
        in property <TrackingView> data;
        in property <[SegmentRow]> segments;
        in property <DashboardBalance> balance;
        in property <[Metric]> summary;
        in property <DashboardBalance> dashboard-balance;
        in property <int> dashboard-range;
        in property <string> dashboard-date;
        in property <bool> confirm-entry-delete;
        in property <int> range;
        in property <string> date;
        in property <bool> wide;
        in property <bool> featured;
        in property <bool> inline-ranges;
        callback command(int);
        callback select-project(int);
        callback create-project(string);
        callback describe(string);
        callback select-range(int);
        callback select-dashboard-range(int);
        callback browse-today;
        callback edit(string);
        callback remove(string);
        callback browse;
        callback find-times;
        callback add-project;
        callback account;

        spacing: 12px;
        if root.featured: HorizontalLayout {
            spacing: 12px;
            TrackingControls {
                data: root.data; featured: true;
                horizontal-stretch: 2;
                command(event) => { root.command(event); }
                select-project(index) => { root.select-project(index); }
                create-project(name) => { root.create-project(name); }
                describe(text) => { root.describe(text); }
            }
            RunningClock { data: root.data; }
        }
        if !root.featured: TrackingControls {
            data: root.data;
            command(event) => { root.command(event); }
            select-project(index) => { root.select-project(index); }
            create-project(name) => { root.create-project(name); }
            describe(text) => { root.describe(text); }
        }
        if !root.featured: RunningClock { data: root.data; }
        HorizontalLayout {
            spacing: 9px;
            ActionButton {
                label: "Zeit korrigieren"; align-left: true; tile: true;
                detail: root.featured ? "Einträge nachtragen oder anpassen" : "";
                icon: root.featured ? "M4 20 L8 19 L20 7 L17 4 L5 16 Z M14 7 L17 10" : "";
                clicked => { root.find-times(); }
            }
            ActionButton {
                label: "Projekt anlegen"; align-left: true; tile: true;
                detail: root.featured ? "Neues Projekt erstellen" : "";
                icon: root.featured ? "M3 6 H10 L12 8 H21 V19 H3 Z M12 10 V17 M9 14 H15" : "";
                clicked => { root.add-project(); }
            }
            ActionButton {
                label: "Arbeitszeitkonto"; align-left: true; tile: true;
                detail: root.featured ? "Soll, Ist und Saldo anzeigen" : "";
                icon: root.featured ? "M12 3 V21 M7 21 H17 M3 7 H21 M6 7 L2 15 H10 Z M18 7 L14 15 H22 Z" : "";
                clicked => { root.account(); }
            }
        }
        if root.wide: HorizontalLayout {
            spacing: 12px;
            TodayTable {
                segments: root.segments; total: root.data.worked; date: root.date;
                browse => { root.browse(); }
                edit(id) => { root.edit(id); }
                remove(id) => { root.remove(id); }
            }
            AccountSnapshot {
                data: root.data; balance: root.balance; range: root.range; summary: root.summary;
                inline-ranges: root.inline-ranges;
                select(index) => { root.select-range(index); }
            }
        }
        if !root.wide: AccountSnapshot {
            data: root.data; balance: root.balance; range: root.range; summary: root.summary;
                inline-ranges: root.inline-ranges;
            select(index) => { root.select-range(index); }
        }
        if !root.wide: RecordedTimes {
            segments: root.segments;
            edit(id, bounds) => { root.edit(id); }
        }
    }

    export component MiniTimer inherits Window {
        title: "Projektzeit – Mini-Timer";
        background: Theme.rail;
        always-on-top: root.pinned;
        width: 380px;
        height: 380px;
        in property <TrackingView> tracking;
        in property <bool> pinned: true;
        in property <string> feedback;
        callback set-pin(bool);
        callback open-main;
        callback tracking-action(int);
        callback tracking-project(int);
        callback quick-project(string);
        callback describe-work(string);

        VerticalLayout {
            padding: 10px;
            spacing: 6px;
            ScrollView {
                content-width: self.visible-width;
                VerticalLayout {
                    alignment: start;
                    spacing: 6px;
                    TrackingControls {
                        data: root.tracking; compact: true;
                        command(event) => { root.tracking-action(event); }
                        select-project(index) => { root.tracking-project(index); }
                        create-project(name) => { root.quick-project(name); }
                        describe(text) => { root.describe-work(text); }
                    }
                    if root.feedback != "": Label { text: root.feedback; color: Theme.rail-ink; wrap: word-wrap; }
                    DayStatus { data: root.tracking; compact: true; }
                }
            }
            HorizontalLayout {
                spacing: 5px;
                ActionButton { label: root.pinned ? "Angeheftet" : "Anheften"; nav: true; selected: root.pinned; clicked => { root.set-pin(!root.pinned); } }
                ActionButton { label: "Hauptfenster"; nav: true; clicked => { root.open-main(); } }
            }
        }
    }

    export component AccountWindow inherits Window {
        title: "Projektzeit";
        preferred-width: 1376px;
        preferred-height: 826px;
        min-width: 500px;
        min-height: 445px;
        default-font-size: 15px;
        background: Theme.rail;

        in property <int> page;
        in property <int> scheme;
        in property <ScheduleInputs> schedule;
        in property <EditFields> dated-draft;
        in property <MiniOptions> mini-options;
        in property <int> range-index;
        in property <string> heading;
        in property <string> subheading;
        in property <string> notice;
        in property <string> feedback;
        in property <string> report-date;
        in property <bool> has-report;
        in property <TrackingView> tracking;
        in property <[Metric]> summary;
        in property <DashboardBalance> dashboard-balance;
        in property <int> dashboard-range;
        in property <string> dashboard-date;
        in property <bool> confirm-entry-delete;
        in property <EntryForm> entry-form;
        in property <bool> show-entry-form;
        in property <ProjectForm> project-form;
        in property <bool> show-project-form;
        in property <[ProjectForm]> project-list;
        in property <[ProjectChoice]> filter-choices;
        in property <[string]> filter-names;
        in property <int> filter-index;
        in property <[ProjectChoice]> assignment-choices;
        in property <[string]> assignment-names;
        in property <int> assignment-index;
        in property <[Metric]> week-metrics;
        in property <[Metric]> month-metrics;
        private property <bool> show-about;
        in property <bool> autostart-supported;
        in property <bool> autostart;
        in property <string> database-path;
        in property <string> opening-summary;
        in property <[AdjustmentRow]> adjustments;
        in property <[DayRow]> rows;
        in property <[SegmentRow]> segments;
        in property <[string]> settings;
        in property <[string]> history;
        in property <string> preview-text;
        in property <bool> preview-ready;
        private property <bool> compact: root.width < 810px;
        private property <bool> wide-overview: root.width >= 1120px;

        callback navigate(int);
        callback select-range(int);
        callback select-dashboard-range(int);
        callback browse-today;
        callback change-scheme(int);
        callback edit-schedule(int, string);
        callback change-mini-option(int, bool);
        callback reset-mini-position;
        callback native-minimized(bool);
        callback native-clock-interrupted;
        callback mini-moved;
        callback mini-closed;
        callback step-period(int);
        callback go-to-date(string);
        callback preview-edit(EditFields);
        callback capture-dated(EditFields);
        callback confirm-edit();
        callback cancel-edit();
        callback export-to(string, string);
        callback tracking-action(int);
        callback tracking-project(int);
        callback quick-project(string);
        callback describe-work(string);
        callback edit-record(string, bool);
        callback request-delete-record(string);
        callback new-record;
        callback save-record(EntryForm);
        callback delete-record;
        callback cancel-record;
        callback filter-project(string);
        callback edit-project(string);
        callback save-project(ProjectForm);
        callback archive-project(string);
        callback cancel-project;
        callback change-autostart(bool);
        callback backup-to(string);
        callback show-mini;

        function apply-scheme() {
            Palette.color-scheme = root.scheme == 1 ? ColorScheme.light
                : root.scheme == 2 ? ColorScheme.dark : ColorScheme.unknown;
        }
        init => { root.apply-scheme(); }
        changed scheme => { root.apply-scheme(); }

        VerticalLayout {
            x: 0px;
            y: 0px;
            width: root.width;
            height: root.height;
            Rectangle {
                height: 48px;
                background: Theme.rail;
                HorizontalLayout {
                    padding-left: 16px; padding-right: 14px;
                    spacing: 10px;
                    LineIcon {
                        width: 28px; height: 28px; ink: Theme.rail-accent;
                        shape: "M12 5 A8 8 0 1 0 12 21 A8 8 0 1 0 12 5 M9 2 H15 M12 2 V5 M17 6 L19 4 M12 9 V14";
                    }
                    Text { text: "Projektzeit"; color: Theme.rail-ink; font-size: 20px; font-weight: 710; vertical-alignment: center; }
                    Rectangle { horizontal-stretch: 1; }
                    ActionButton { label: "Mini-Timer"; nav: true; clicked => { root.show-mini(); } }
                }
            }
            if root.compact: PageNav {
                compact: true; page: root.page;
                navigate(index) => { root.navigate(index); }
            }
            HorizontalLayout {
                if !root.compact: PageNav {
                    width: 174px;
                    page: root.page;
                    navigate(index) => { root.navigate(index); }
                }
                Rectangle {
                    horizontal-stretch: 1;
                    background: Theme.canvas;
                    border-radius: 14px;
                    VerticalLayout {
                    padding: root.compact ? 13px : 22px;
                    spacing: 14px;
                    HorizontalLayout {
                        spacing: 13px;
                        if root.page == 0: GreetingMark {}
                        VerticalLayout {
                            spacing: 2px;
                            Text { text: root.heading; color: Theme.ink; font-size: root.page == 0 ? 34px : 28px; font-weight: 735; }
                            if root.subheading != "": Label { text: root.subheading; font-size: root.page == 0 ? 15px : 14px; wrap: word-wrap; }
                        }
                        Rectangle { horizontal-stretch: 1; }
                    }
                    if root.feedback != "": Label { text: root.feedback; color: Theme.green; wrap: word-wrap; }

                    if root.page == 1 || root.page == 2: VerticalLayout {
                        spacing: 8px;
                        if root.page == 1: HorizontalLayout {
                            spacing: 6px;
                            for label[index] in ["Tag", "Woche", "Monat"]: ActionButton {
                                label: label; selected: index == root.range-index;
                                clicked => { root.select-range(index); }
                            }
                            Rectangle { horizontal-stretch: 1; }
                        }
                        HorizontalLayout {
                            spacing: 6px;
                            ActionButton { label: "Zurück"; clicked => { root.step-period(-1); } }
                            ActionButton { label: "Weiter"; clicked => { root.step-period(1); } }
                            Label { text: "Stichtag"; vertical-alignment: center; horizontal-stretch: 0; }
                            date := LineEdit {
                                text: root.report-date;
                                placeholder-text: "TT.MM.JJJJ";
                                accessible-label: "Stichtag";
                                min-width: 96px; preferred-width: 128px; horizontal-stretch: 0;
                                accepted => { root.go-to-date(self.text); }
                            }
                            ActionButton { label: "Anzeigen"; clicked => { root.go-to-date(date.text); } }
                            Rectangle { horizontal-stretch: 1; }
                        }
                    }

                    ScrollView {
                        content-width: self.visible-width;
                        VerticalLayout {
                            alignment: start;
                            spacing: 14px;

                            if root.page == 0: TodayDashboard {
                                data: root.tracking; segments: root.segments;
                                wide: root.wide-overview; featured: root.width >= 970px;
                                inline-ranges: root.width >= 1470px || (!root.wide-overview && root.width >= 640px);
                                balance: root.dashboard-balance; range: root.dashboard-range;
                                date: root.dashboard-date; summary: root.summary;
                                command(event) => { root.tracking-action(event); }
                                select-project(index) => { root.tracking-project(index); }
                                create-project(name) => { root.quick-project(name); }
                                describe(text) => { root.describe-work(text); }
                                select-range(range) => { root.select-dashboard-range(range); }
                                edit(id) => { root.edit-record(id, false); }
                                remove(id) => { root.request-delete-record(id); }
                                browse => { root.browse-today(); }
                                find-times => { root.new-record(); }
                                add-project => { root.navigate(3); root.edit-project(""); }
                                account => { root.navigate(2); }
                            }
                            if (root.page == 1 || root.page == 2) && root.notice != "":
                                Label { text: root.notice; wrap: word-wrap; }
                            if root.page == 1: HorizontalLayout {
                                spacing: 8px;
                                ActionButton { label: "+ Zeit hinzufügen"; primary: true; clicked => { root.new-record(); } }
                                filter := ComboBox {
                                    model: root.filter-names; current-index: root.filter-index;
                                    accessible-label: "Nach Projekt filtern"; horizontal-stretch: 1;
                                    selected => { root.filter-project(root.filter-choices[self.current-index].id); }
                                }
                            }
                            if root.page == 1 && root.show-entry-form: RecordEditor {
                                data: root.entry-form; choices: root.assignment-choices;
                                names: root.assignment-names; selection: root.assignment-index;
                                propose-delete: root.confirm-entry-delete;
                                save(fields) => { root.save-record(fields); }
                                cancel => { root.cancel-record(); }
                                remove => { root.delete-record(); }
                            }
                            if root.page == 1: RecordedTimes {
                                segments: root.segments; history: true;
                                edit(id, bounds) => { root.edit-record(id, bounds); }
                            }
                            if root.page == 1 && root.rows.length > 0: Card {
                                VerticalLayout {
                                    padding: 14px;
                                    spacing: 4px;
                                    DayLine { day: { date: "Datum", actual: "Ist netto", target: "Soll", delta: "Tagessaldo" }; header: true; }
                                    for day in root.rows: DayLine { day: day; }
                                }
                            }
                            if root.page == 1: Card {
                                VerticalLayout {
                                    padding: 14px;
                                    spacing: 8px;
                                    SectionTitle { text: "Zeiten exportieren"; }
                                    template := LineEdit {
                                        accessible-label: "Pfad zur Excel-Vorlage";
                                        placeholder-text: "Pfad zur Excel-Vorlage (optional)";
                                    }
                                    Label { text: "Die Vorlagenkopie behält vorhandene Arbeitsblätter."; wrap: word-wrap; }
                                    HorizontalLayout {
                                        spacing: 8px;
                                        file := LineEdit {
                                            accessible-label: "Zielpfad für XLSX";
                                            placeholder-text: "Zielpfad für XLSX"; horizontal-stretch: 1;
                                        }
                                        ActionButton { label: "XLSX exportieren"; clicked => { root.export-to(file.text, template.text); } }
                                    }
                                }
                            }
                            if root.page == 2 && root.week-metrics.length > 0: BalancePanel {
                                heading: "Woche"; metrics: root.week-metrics;
                            }
                            if root.page == 2 && root.month-metrics.length > 0: BalancePanel {
                                heading: "Monat und Gesamtsaldo"; metrics: root.month-metrics;
                            }
                            if root.page == 2 && root.has-report: Card {
                                HorizontalLayout {
                                    padding: 14px; spacing: 8px;
                                    account-file := LineEdit { accessible-label: "Zielpfad für Kontobericht"; placeholder-text: "Zielpfad für Kontobericht"; }
                                    ActionButton { label: "Konto exportieren"; clicked => { root.export-to(account-file.text, ""); } }
                                }
                            }
                            if root.page == 2: AccountSettings {
                                date: root.report-date; corrections: true; opening-summary: root.opening-summary;
                                adjustments: root.adjustments; settings: root.settings; history: root.history;
                                preview-text: root.preview-text; preview-ready: root.preview-ready;
                                preview-edit(fields) => { root.preview-edit(fields); }
                                confirm-edit => { root.confirm-edit(); }
                                cancel-edit => { root.cancel-edit(); }
                            }
                            if root.page == 3: ProjectManager {
                                projects: root.project-list; data: root.project-form; editing: root.show-project-form;
                                edit(id) => { root.edit-project(id); }
                                save(fields) => { root.save-project(fields); }
                                archive(id) => { root.archive-project(id); }
                                cancel => { root.cancel-project(); }
                            }
                            if root.page == 3: AccountSettings {
                                scheme: root.scheme; date: root.report-date;
                                schedule: root.schedule; mini-options: root.mini-options;
                                dated-draft: root.dated-draft;
                                capture-dated(fields) => { root.capture-dated(fields); }
                                edit-schedule(index, value) => { root.edit-schedule(index, value); }
                                change-mini-option(index, value) => { root.change-mini-option(index, value); }
                                reset-mini-position => { root.reset-mini-position(); }
                                settings: root.settings; history: root.history;
                                preview-text: root.preview-text; preview-ready: root.preview-ready;
                                change-scheme(index) => { root.change-scheme(index); }
                                preview-edit(fields) => { root.preview-edit(fields); }
                                confirm-edit => { root.confirm-edit(); }
                                cancel-edit => { root.cancel-edit(); }
                            }
                            if root.page == 3: Card {
                                VerticalLayout {
                                    padding: 14px; spacing: 9px;
                                    SectionTitle { text: "Daten und Programm"; }
                                    if root.autostart-supported: CheckBox {
                                        text: "Beim Anmelden starten"; checked: root.autostart;
                                        toggled => { root.change-autostart(self.checked); }
                                    }
                                    Label { text: root.database-path; wrap: word-wrap; }
                                    HorizontalLayout {
                                        spacing: 8px;
                                        backup := LineEdit { accessible-label: "Zielpfad für Datensicherung"; placeholder-text: "Zielpfad für SQLite-Sicherung"; }
                                        ActionButton { label: "Sichern"; clicked => { root.backup-to(backup.text); } }
                                    }
                                    ActionButton { label: root.show-about ? "Über Projektzeit schließen" : "Über Projektzeit"; clicked => { root.show-about = !root.show-about; } }
                                    if root.show-about: AboutSlint {}
                                }
                            }
                        }
                    }
                    }
                }
            }
        }
    }
}
