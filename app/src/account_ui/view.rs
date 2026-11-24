slint::slint! {
    #[edition = "1.19"]
    import { AboutSlint, CheckBox, ComboBox, LineEdit, Palette, ScrollView } from "std-widgets.slint";

    export struct ScheduleInputs { values: [string], message: string, invalid: bool, date: string }
    export struct MiniOptions { auto-show: bool, topmost: bool, remember: bool }
    export struct Metric { label: string, value: string }
    export struct DayRow { date: string, actual: string, target: string, delta: string }
    export struct SegmentRow {
        title: string, detail: string, time: string, begin: string, finish: string,
        amount: string, pending: bool, pause: bool,
    }
    export struct AdjustmentRow { id: string, date: string, reason: string, amount: string, voided: bool }
    export struct ChartDay { label: string, actual: float, target: float }
    export struct Overview {
        configured: bool, worked: string, remaining: string, finish: string,
        daily-target: string, progress-label: string, progress: float, breaks: string,
        project: string, balances: [Metric], week: [Metric], month: [Metric],
        chart: [ChartDay], week-title: string, week-dates: string, month-title: string,
        notice: string,
    }
    export struct ProjectRow { name: string, code: string, customer: string, archived: bool, swatch: color }
    export struct EditFields {
        kind: int, date: string, amount: string,
        monday: string, tuesday: string, wednesday: string, thursday: string,
        friday: string, saturday: string, sunday: string,
        reason: string, entry-id: string,
    }

    global Theme {
        private property <color> system-background: Palette.background;
        out property <bool> dark: system-background.red < 128;
        out property <color> canvas: dark ? #13251f : #f3f7fb;
        out property <color> surface: dark ? #1b2f28 : #ffffff;
        out property <color> tint: dark ? #253f33 : #ecfdf5;
        out property <color> border: dark ? #3b5145 : #dbe5ed;
        out property <color> ink: dark ? #f2f5ed : #0f172a;
        out property <color> muted: dark ? #b2c4b8 : #46576a;
        out property <color> green: dark ? #a0edc2 : #0e5a47;
        out property <color> focus: dark ? #b8e9bf : #0e5a47;
        out property <color> warning: dark ? #f2cd91 : #764718;
        out property <color> rail: #0B1f1a;
        out property <color> rail-active: #0e774b;
        out property <color> rail-ink: #f3f9ed;
        out property <color> rail-muted: #bad1c0;
        out property <color> chart: dark ? #34d399 : #0B9b57;
        out property <color> track: dark ? #345747 : #dbe7ee;
        out property <color> pause-tint: dark ? #43392a : #fff5dc;
        out property <color> hero: dark ? #234936 : #28563d;
        out property <color> on-hero: #f4faef;
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
        callback clicked;

        min-width: caption.preferred-width + 28px + (root.icon == "" ? 0px : 26px);
        height: 38px;
        horizontal-stretch: root.align-left ? 1 : 0;
        border-radius: 7px;
        border-width: focus.has-focus ? 2px : 1px;
        border-color: focus.has-focus ? (root.nav ? Theme.rail-ink : Theme.focus)
            : root.nav || root.primary ? transparent : Theme.border;
        background: !root.enabled ? Theme.tint
            : root.nav ? (root.selected || touch.has-hover ? Theme.rail-active : transparent)
            : root.primary ? (touch.has-hover ? Theme.green.darker(0.12) : Theme.green)
            : touch.has-hover || root.selected ? Theme.tint : Theme.surface;
        forward-focus: focus;

        accessible-role: button;
        accessible-label: root.label;
        accessible-enabled: root.enabled;
        accessible-checkable: root.nav;
        accessible-checked: root.selected;
        accessible-action-default => { if (root.enabled) { root.clicked(); } }

        if root.icon != "": LineIcon {
            x: 13px; y: (parent.height - self.height)/2; width: 18px; height: 18px;
            shape: root.icon; ink: Theme.rail-ink;
        }
        caption := Text {
            x: root.icon == "" ? 14px : 39px;
            width: parent.width - self.x - 14px;
            height: parent.height;
            text: root.label;
            color: !root.enabled ? Theme.muted : root.nav ? Theme.rail-ink
                : root.primary ? Theme.on-action : Theme.ink;
            font-size: 14px;
            font-weight: root.selected || root.primary ? 670 : 520;
            vertical-alignment: center;
            horizontal-alignment: root.align-left ? left : center;
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

    component StatCard inherits Card {
        in property <Metric> item;
        in property <bool> hero;
        in property <float> progress;
        in property <string> progress-label;

        min-width: 170px;
        preferred-height: 139px;
        horizontal-stretch: 1;
        background: root.hero ? Theme.hero : Theme.surface;
        border-color: root.hero ? Theme.hero : Theme.border;
        VerticalLayout {
            padding: 16px;
            spacing: 6px;
            alignment: start;
            Label { text: root.item.label; color: root.hero ? Theme.rail-muted : Theme.muted; }
            Text {
                text: root.item.value;
                font-size: 28px;
                font-weight: 700;
                color: root.hero ? Theme.on-hero : Theme.ink;
            }
            if root.progress-label != "": VerticalLayout {
                spacing: 7px;
                Rectangle {
                    height: 5px;
                    border-radius: 3px;
                    background: #ffffff38;
                    Rectangle {
                        x: 0px;
                        y: 0px;
                        width: parent.width * root.progress;
                        height: parent.height;
                        border-radius: parent.border-radius;
                        background: #b5e0b6;
                    }
                }
                Label { text: root.progress-label; color: Theme.rail-muted; font-size: 13px; }
            }
        }
    }

    component StatCards inherits VerticalLayout {
        in property <[Metric]> items;
        in property <bool> narrow;
        in property <float> progress;
        in property <string> progress-label;
        spacing: 8px;
        if !root.narrow: HorizontalLayout {
            spacing: 12px;
            for item[index] in root.items: StatCard {
                item: item;
                hero: index == 0;
                progress: root.progress;
                progress-label: index == 0 ? root.progress-label : "";
            }
        }
        if root.narrow: VerticalLayout {
            spacing: 8px;
            for item[index] in root.items: StatCard {
                item: item;
                hero: index == 0;
                progress: root.progress;
                progress-label: index == 0 ? root.progress-label : "";
                preferred-height: root.progress-label != "" && index == 0 ? 126px : 90px;
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

    component TimelinePanel inherits Card {
        in property <[SegmentRow]> segments;
        in property <string> heading;
        VerticalLayout {
            padding: 18px;
            spacing: 11px;
            alignment: start;
            SectionTitle { text: root.heading; }
            if root.segments.length == 0: Label { text: "Für diesen Tag sind keine Zeiten erfasst."; wrap: word-wrap; }
            for segment in root.segments: VerticalLayout {
                spacing: 8px;
                Rectangle { height: 1px; background: Theme.border; }
                HorizontalLayout {
                    alignment: stretch;
                    spacing: 10px;
                    VerticalLayout {
                        horizontal-stretch: 1;
                        spacing: 3px;
                        Text { text: segment.title; color: Theme.ink; font-weight: 650; wrap: word-wrap; }
                        if segment.detail != "": Label {
                            text: segment.detail;
                            color: segment.pending ? Theme.warning : Theme.muted;
                            wrap: word-wrap;
                            font-size: 13px;
                        }
                    }
                    Label { text: segment.time; font-size: 13px; horizontal-stretch: 0; }
                    Text {
                        text: segment.amount;
                        color: Theme.ink;
                        font-size: 13px;
                        font-weight: 660;
                        min-width: 68px;
                        horizontal-alignment: right;
                        horizontal-stretch: 0;
                    }
                }
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

    component ProjectList inherits Card {
        in property <[ProjectRow]> projects;
        VerticalLayout {
            padding: 18px;
            spacing: 10px;
            alignment: start;
            if root.projects.length == 0: Label { text: "Es sind noch keine Projekte erfasst."; }
            for project in root.projects: VerticalLayout {
                spacing: 10px;
                HorizontalLayout {
                    spacing: 16px;
                    Rectangle {
                        width: 5px;
                        height: 30px;
                        border-radius: 3px;
                        background: project.swatch.alpha > 0 ? project.swatch : project.archived ? Theme.muted : Theme.green;
                    }
                    VerticalLayout {
                        horizontal-stretch: 1;
                        spacing: 3px;
                        Text { text: project.name; color: Theme.ink; font-weight: 670; wrap: word-wrap; }
                        if project.code != "": Label { text: project.code; }
                    }
                    Label { text: project.customer; wrap: word-wrap; }
                    Label { text: project.archived ? "Archiviert" : "Aktiv"; horizontal-stretch: 0; }
                }
                Rectangle { height: 1px; background: Theme.border; }
            }
        }
    }

    component PageNav inherits Rectangle {
        in property <int> page;
        in property <bool> compact;
        callback navigate(int);
        property <[string]> links: ["Dashboard", "Zeiterfassung", "Projekte", "Arbeitszeitkonto",
            "Berichte & Export", "Korrekturen", "Einstellungen"];
        property <[string]> icons: [
            "M3 11 L12 3 L21 11 M5 10 V21 H10 V15 H14 V21 H19 V10",
            "M12 3 A9 9 0 1 0 12 21 A9 9 0 1 0 12 3 M10 8 L16 12 L10 16 Z",
            "M3 7 L12 3 L21 7 L12 11 Z M3 12 L12 16 L21 12 M3 17 L12 21 L21 17",
            "M4 20 V13 H7 V20 Z M10 20 V9 H13 V20 Z M16 20 V4 H19 V20 Z",
            "M6 3 H15 L19 7 V21 H6 Z M15 3 V8 H19 M9 12 H16 M9 15 H16",
            "M4 20 L8 19 L20 7 L17 4 L5 16 Z M14 7 L17 10",
            "M12 3 L14 6 L17 5 L19 7 L18 10 L21 12 L18 14 L19 17 L17 19 L14 18 L12 21 L10 18 L7 19 L5 17 L6 14 L3 12 L6 10 L5 7 L7 5 L10 6 Z M12 9 A3 3 0 1 0 12 15 A3 3 0 1 0 12 9",
        ];
        property <[string]> short-links: ["Dashboard", "Zeiten", "Projekte", "Zeitkonto", "Berichte", "Korrekturen", "Einstellungen"];
        background: Theme.rail;

        if !root.compact: VerticalLayout {
            padding: 18px;
            spacing: 7px;
            HorizontalLayout {
                spacing: 9px;
                LineIcon {
                    width: 29px; height: 29px; ink: #34d399;
                    shape: "M12 5 A8 8 0 1 0 12 21 A8 8 0 1 0 12 5 M9 2 H15 M12 2 V5 M17 6 L19 4 M12 9 V14";
                }
                Label { text: "Projektzeit"; color: Theme.rail-ink; font-size: 22px; font-weight: 720; vertical-alignment: center; }
            }
            Rectangle { height: 12px; }
            for link[index] in root.links: ActionButton {
                label: link; nav: true; align-left: true; selected: index == root.page; icon: root.icons[index];
                clicked => { root.navigate(index); }
            }
            Rectangle { vertical-stretch: 1; }
            ActionButton {
                label: "Über Projektzeit"; nav: true; align-left: true; selected: root.page == 7;
                clicked => { root.navigate(7); }
            }
        }
        if root.compact: VerticalLayout {
            padding: 8px;
            spacing: 3px;
            HorizontalLayout {
                Label { text: "Projektzeit"; color: Theme.rail-ink; font-size: 19px; font-weight: 700; vertical-alignment: center; }
                Rectangle { horizontal-stretch: 1; }
                ActionButton { label: "Über"; nav: true; selected: root.page == 7; clicked => { root.navigate(7); } }
            }
            HorizontalLayout {
                spacing: 2px;
                for index in 4: ActionButton {
                    label: root.short-links[index]; nav: true; selected: index == root.page;
                    horizontal-stretch: 1;
                    clicked => { root.navigate(index); }
                }
            }
            HorizontalLayout {
                spacing: 2px;
                for index in 3: ActionButton {
                    label: root.short-links[index + 4]; nav: true; selected: index + 4 == root.page;
                    horizontal-stretch: 1;
                    clicked => { root.navigate(index + 4); }
                }
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
                SectionTitle { text: root.corrections ? "Korrektur erfassen" : "Datierte Arbeitszeitregeln"; }
                kind := ComboBox {
                    property <int> operation: self.current-index + (root.corrections ? 4 : 0);
                    model: root.corrections
                        ? ["Neue Korrektur", "Korrektur bearbeiten", "Korrektur stornieren"]
                        : ["Wochenplan mit Gültigkeitsdatum", "Startsaldo", "Tagessoll setzen", "Tagessoll zurücksetzen"];
                    current-index: root.corrections ? 0 : root.dated-draft.kind;
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

    component DashboardValue inherits VerticalLayout {
        in property <string> label;
        in property <string> value;
        in property <string> detail;
        horizontal-stretch: 1;
        alignment: start;
        spacing: 4px;
        Label { text: root.label; color: Theme.ink; font-weight: 650; font-size: 13px; wrap: word-wrap; }
        Text { text: root.value; color: Theme.ink; font-weight: 730; font-size: 23px; }
        if root.detail != "": Label { text: root.detail; font-size: 12px; }
    }

    component DashboardNumbers inherits Card {
        in property <Overview> data;
        HorizontalLayout {
            padding: 14px;
            spacing: 14px;
            VerticalLayout {
                spacing: 10px;
                HorizontalLayout {
                    spacing: 12px;
                    DashboardValue { label: "Arbeitszeit heute"; value: root.data.worked; detail: root.data.daily-target; }
                    Rectangle { width: 1px; background: Theme.border; }
                    DashboardValue { label: "Noch zu arbeiten"; value: root.data.remaining; }
                    Rectangle { width: 1px; background: Theme.border; }
                    DashboardValue { label: "Vorauss. Feierabend"; value: root.data.finish; }
                }
                HorizontalLayout {
                    spacing: 10px;
                    alignment: stretch;
                    ProgressTrack { progress: root.data.progress; horizontal-stretch: 1; }
                    Label { text: root.data.progress-label; vertical-alignment: center; horizontal-stretch: 0; }
                }
            }
        }
    }


    component CurrentProject inherits Card {
        in property <Overview> data;
        VerticalLayout {
            alignment: start;
            padding: 14px;
            spacing: 8px;
            SectionTitle { text: "Aktuelles Projekt"; }
            Label { text: root.data.project; color: Theme.ink; font-size: 13px; wrap: word-wrap; }
            Label { text: "Pausen heute: " + root.data.breaks; font-size: 12px; }
        }
    }

    component AccountOverview inherits Card {
        in property <[Metric]> values;
        VerticalLayout {
            padding: 14px;
            spacing: 10px;
            alignment: start;
            SectionTitle { text: "Arbeitszeitkonto – Überblick"; }
            if root.values.length == 0: Label { text: "Noch kein Arbeitszeitkonto eingerichtet."; wrap: word-wrap; }
            HorizontalLayout {
                spacing: 7px;
                for metric in root.values: Rectangle {
                    horizontal-stretch: 1;
                    background: Theme.tint;
                    border-radius: 7px;
                    VerticalLayout {
                        padding: 8px;
                        spacing: 4px;
                        Label { text: metric.label; color: Theme.ink; font-size: 12px; wrap: word-wrap; }
                        Text { text: metric.value; color: Theme.green; font-weight: 730; font-size: 18px; }
                    }
                }
            }
        }
    }

    component WeekCard inherits Card {
        in property <Overview> data;
        VerticalLayout {
            padding: 14px;
            spacing: 7px;
            alignment: start;
            HorizontalLayout {
                spacing: 8px;
                SectionTitle { text: root.data.week-title; }
                Label { text: root.data.week-dates; font-size: 12px; vertical-alignment: center; }
            }
            if root.data.week.length == 0: Label { text: "Noch keine Wochenwerte."; }
            if root.data.week.length > 0: HorizontalLayout {
                spacing: 10px;
                VerticalLayout {
                    horizontal-stretch: 1;
                    alignment: start;
                    spacing: 5px;
                    for metric in root.data.week: HorizontalLayout {
                        spacing: 5px;
                        Label { text: metric.label; font-size: 12px; horizontal-stretch: 1; }
                        Text { text: metric.value; color: Theme.ink; font-size: 13px; font-weight: 700; }
                    }
                }
                VerticalLayout {
                    spacing: 4px;
                    HorizontalLayout {
                        spacing: 5px;
                        for day in root.data.chart: VerticalLayout {
                            spacing: 3px;
                            Rectangle {
                                width: 13px;
                                height: 49px;
                                Rectangle {
                                    y: parent.height - self.height;
                                    height: parent.height * day.target;
                                    background: Theme.track;
                                    border-radius: 2px;
                                }
                                Rectangle {
                                    width: parent.width - 4px;
                                    y: parent.height - self.height;
                                    height: parent.height * day.actual;
                                    background: Theme.chart;
                                    border-radius: 2px;
                                }
                            }
                            Label { text: day.label; font-size: 10px; horizontal-alignment: center; }
                        }
                    }
                    Label { text: "Ist (grün) · Soll"; font-size: 11px; horizontal-alignment: center; }
                }
            }
        }
    }

    component DashboardEntry inherits Rectangle {
        in property <SegmentRow> entry;
        in property <bool> header;
        height: header ? 28px : 33px;
        background: header ? Theme.canvas : entry.pause ? Theme.pause-tint : Theme.surface;
        border-radius: 4px;
        HorizontalLayout {
            padding-left: 8px;
            padding-right: 8px;
            spacing: 7px;
            Text {
                text: root.entry.title; color: Theme.ink; font-size: 12px; font-weight: root.header ? 730 : 600;
                min-width: 74px; preferred-width: 124px; overflow: elide; vertical-alignment: center;
            }
            Text {
                text: root.entry.detail; color: root.entry.pending ? Theme.warning : Theme.ink; font-size: 12px;
                min-width: 84px; horizontal-stretch: 1; overflow: elide; vertical-alignment: center;
                accessible-description: root.entry.detail;
            }
            for value in [root.entry.begin, root.entry.finish, root.entry.amount]: Text {
                text: value; color: Theme.ink; font-size: 12px; min-width: 42px;
                vertical-alignment: center; horizontal-alignment: right;
            }
        }
    }

    component DashboardTimeline inherits Card {
        in property <[SegmentRow]> segments;
        VerticalLayout {
            padding: 13px;
            spacing: 5px;
            alignment: start;
            SectionTitle { text: "Heutige Zeiterfassung"; }
            if root.segments.length == 0: Label { text: "Heute sind noch keine Zeiten erfasst."; }
            if root.segments.length > 0: DashboardEntry {
                header: true;
                entry: { title: "Projekt", detail: "Tätigkeit", begin: "Beginn", finish: "Ende", amount: "Dauer" };
            }
            for segment in root.segments: DashboardEntry { entry: segment; }
        }
    }


    component DashboardBody inherits VerticalLayout {
        in property <Overview> data;
        in property <[SegmentRow]> segments;
        in property <bool> stacked;
        callback navigate(int);
        callback show-mini;
        spacing: 12px;
        alignment: start;

        HorizontalLayout {
            spacing: 12px;
            DashboardNumbers { data: root.data; horizontal-stretch: 3; }
            if !root.stacked: CurrentProject { data: root.data; horizontal-stretch: 1; min-width: 174px; }
        }
        if root.stacked: CurrentProject { data: root.data; }
        if root.data.notice != "": Label { text: root.data.notice; font-size: 12px; wrap: word-wrap; }

        if !root.stacked: HorizontalLayout {
            spacing: 12px;
            AccountOverview { values: root.data.balances; horizontal-stretch: 1; }
            WeekCard { data: root.data; horizontal-stretch: 1; }
        }
        if root.stacked: AccountOverview { values: root.data.balances; }
        if root.stacked: WeekCard { data: root.data; }

        if !root.stacked: HorizontalLayout {
            spacing: 12px;
            DashboardTimeline { segments: root.segments; horizontal-stretch: 3; }
            BalancePanel { metrics: root.data.month; heading: root.data.month-title; horizontal-stretch: 2; max-width: 348px; }
        }
        if root.stacked: DashboardTimeline { segments: root.segments; }
        if root.stacked: BalancePanel { metrics: root.data.month; heading: root.data.month-title; }

        Card {
            VerticalLayout {
                padding: 12px;
                spacing: 8px;
                SectionTitle { text: "Schnellaktionen"; }
                HorizontalLayout {
                    spacing: 7px;
                    ActionButton { label: "Mini-Timer"; primary: true; clicked => { root.show-mini(); } }
                    ActionButton { label: "Zeiten ansehen"; clicked => { root.navigate(1); } }
                    ActionButton { label: "Korrektur anlegen"; clicked => { root.navigate(5); } }
                    Rectangle { horizontal-stretch: 1; }
                }
            }
        }
    }

    export component MiniTimer inherits Window {
        title: "Projektzeit – Mini-Timer";
        background: Theme.rail;
        always-on-top: root.pinned;
        width: 352px;
        height: 185px;
        in property <Overview> overview;
        in property <bool> pinned: true;
        callback set-pin(bool);
        callback open-main;
        callback refresh;

        Timer {
            interval: 30000ms;
            triggered => { root.refresh(); }
        }
        VerticalLayout {
            padding: 13px;
            spacing: 10px;
            HorizontalLayout {
                spacing: 4px;
                for metric in [
                    { label: "Heute", value: root.overview.worked },
                    { label: "Noch", value: root.overview.remaining },
                    { label: "Feierabend", value: root.overview.finish }
                ]: VerticalLayout {
                    spacing: 5px;
                    Label { text: metric.label; color: Theme.rail-muted; font-size: 12px; }
                    Text { text: metric.value; color: Theme.rail-ink; font-weight: 720; font-size: 19px; }
                }
            }
            ProgressTrack { progress: root.overview.progress; background: #174b3d; }
            Label { text: root.overview.project; color: Theme.rail-ink; font-size: 13px; }
            HorizontalLayout {
                spacing: 6px;
                ActionButton {
                    label: root.pinned ? "Angeheftet" : "Anheften";
                    nav: true; selected: root.pinned;
                    clicked => { root.set-pin(!root.pinned); }
                }
                ActionButton { label: "Hauptfenster"; nav: true; clicked => { root.open-main(); } }
                ActionButton { label: "Neu laden"; nav: true; clicked => { root.refresh(); } }
            }
        }
    }

    export component AccountWindow inherits Window {
        title: "Projektzeit";
        preferred-width: 1100px;
        preferred-height: 760px;
        min-width: 500px;
        min-height: 445px;
        default-font-size: 15px;
        background: Theme.canvas;

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
        in property <Overview> overview;
        in property <[AdjustmentRow]> adjustments;
        in property <[Metric]> cards;
        in property <[Metric]> metrics;
        in property <[DayRow]> rows;
        in property <[SegmentRow]> segments;
        in property <[ProjectRow]> projects;
        in property <[string]> settings;
        in property <[string]> history;
        in property <string> preview-text;
        in property <bool> preview-ready;
        private property <bool> compact: root.width < 900px;
        private property <bool> narrow: root.width < 640px;

        callback navigate(int);
        callback select-range(int);
        callback change-scheme(int);
        callback edit-schedule(int, string);
        callback change-mini-option(int, bool);
        callback reset-mini-position;
        callback native-minimized(bool);
        callback mini-moved;
        callback mini-closed;
        callback step-period(int);
        callback go-to-date(string);
        callback refresh();
        callback preview-edit(EditFields);
        callback capture-dated(EditFields);
        callback confirm-edit();
        callback cancel-edit();
        callback export-to(string);
        callback show-mini;
        callback refresh-mini;

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
            if root.compact: PageNav {
                compact: true; page: root.page;
                navigate(index) => { root.navigate(index); }
            }
            HorizontalLayout {
                if !root.compact: PageNav {
                    width: 200px;
                    page: root.page;
                    navigate(index) => { root.navigate(index); }
                }
                VerticalLayout {
                    horizontal-stretch: 1;
                    padding: root.compact ? 14px : 24px;
                    spacing: 16px;
                    HorizontalLayout {
                        spacing: 10px;
                        VerticalLayout {
                            spacing: 3px;
                            if root.page == 0: Label { text: root.subheading; font-size: 12px; }
                            Text { text: root.heading; color: Theme.ink; font-size: 28px; font-weight: 720; }
                            if root.page != 0 && root.subheading != "": Label { text: root.subheading; wrap: word-wrap; }
                        }
                        Rectangle { horizontal-stretch: 1; }
                        if root.page < 5: ActionButton { label: "Aktualisieren"; clicked => { root.refresh(); } }
                    }
                    if root.feedback != "": Label { text: root.feedback; color: Theme.green; wrap: word-wrap; }

                    if root.page == 1 || root.page == 3 || root.page == 4: VerticalLayout {
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

                            if root.page == 0: DashboardBody {
                                data: root.overview;
                                segments: root.segments;
                                stacked: root.compact;
                                navigate(index) => { root.navigate(index); }
                                show-mini => { root.show-mini(); }
                            }
                            if (root.page == 1 || root.page == 3 || root.page == 4) && root.has-report: StatCards {
                                items: root.cards;
                                narrow: root.narrow;
                            }

                            if !root.has-report && (root.page == 0 || root.page == 1 || root.page == 3 || root.page == 4): Card {
                                background: Theme.tint;
                                VerticalLayout {
                                    padding: 16px;
                                    spacing: 8px;
                                    SectionTitle { text: "Arbeitszeitkonto einrichten"; }
                                    Label { text: root.notice; wrap: word-wrap; }
                                    HorizontalLayout {
                                        ActionButton { label: "Einstellungen öffnen"; primary: true; clicked => { root.navigate(6); } }
                                        Rectangle { horizontal-stretch: 1; }
                                    }
                                }
                            }
                            if root.has-report && root.notice != "": Label { text: root.notice; wrap: word-wrap; }

                            if root.page == 1 && root.range-index == 0: TimelinePanel { segments: root.segments; heading: "Tagesverlauf"; }
                            if root.page == 1 && root.rows.length > 0: Card {
                                VerticalLayout {
                                    padding: 16px;
                                    spacing: 4px;
                                    DayLine { day: { date: "Datum", actual: "Ist netto", target: "Soll", delta: "Differenz" }; header: true; }
                                    for day in root.rows: DayLine { day: day; }
                                }
                            }
                            if root.has-report && (root.page == 1 || root.page == 3 || root.page == 4): BalancePanel {
                                metrics: root.metrics;
                                heading: root.page == 1 ? "Zeitraumsaldo" : "Arbeitszeitkonto";
                            }
                            if root.has-report && (root.page == 1 || root.page == 3 || root.page == 4): Card {
                                HorizontalLayout {
                                    padding: 14px;
                                    spacing: 8px;
                                    file := LineEdit {
                                        accessible-label: "XLSX-Dateipfad";
                                        placeholder-text: "Dateipfad für XLSX";
                                        min-width: 140px; horizontal-stretch: 1;
                                    }
                                    ActionButton { label: "XLSX exportieren"; clicked => { root.export-to(file.text); } }
                                }
                            }

                            if root.page == 2: ProjectList { projects: root.projects; }
                            if root.page == 5: AccountSettings {
                                date: root.report-date;
                                corrections: true;
                                adjustments: root.adjustments;
                                history: root.history;
                                preview-text: root.preview-text; preview-ready: root.preview-ready;
                                preview-edit(fields) => { root.preview-edit(fields); }
                                confirm-edit => { root.confirm-edit(); }
                                cancel-edit => { root.cancel-edit(); }
                            }
                            if root.page == 6: AccountSettings {
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
                            if root.page == 7: Card {
                                VerticalLayout { padding: 18px; AboutSlint {} }
                            }
                        }
                    }
                }
            }
        }
    }
}
