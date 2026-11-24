slint::slint! {
    #[edition = "1.19"]
    import { AboutSlint, ComboBox, LineEdit, Palette, ScrollView } from "std-widgets.slint";

    export struct Metric { label: string, value: string }
    export struct DayRow { date: string, actual: string, target: string, delta: string }
    export struct SegmentRow { title: string, detail: string, time: string, amount: string, pending: bool }
    export struct ProjectRow { name: string, code: string, customer: string, archived: bool }
    export struct EditFields {
        kind: int, date: string, amount: string,
        monday: string, tuesday: string, wednesday: string, thursday: string,
        friday: string, saturday: string, sunday: string,
        reason: string, entry-id: string,
    }

    global Theme {
        private property <color> system-background: Palette.background;
        out property <bool> dark: system-background.red < 128;
        out property <color> canvas: dark ? #15231d : #f6f4eb;
        out property <color> surface: dark ? #1d2d25 : #fffdf7;
        out property <color> tint: dark ? #263d31 : #e8f0e4;
        out property <color> border: dark ? #3b5145 : #d9dfd1;
        out property <color> ink: dark ? #f2f5ed : #172d24;
        out property <color> muted: dark ? #b2c4b8 : #4c6053;
        out property <color> green: dark ? #9bd0a9 : #285c43;
        out property <color> focus: dark ? #b8e9bf : #397352;
        out property <color> warning: dark ? #f2cd91 : #764718;
        out property <color> rail: #17362a;
        out property <color> rail-active: #315d47;
        out property <color> rail-ink: #f3f9ed;
        out property <color> rail-muted: #bad1c0;
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
        font-size: 18px;
        font-weight: 680;
    }

    component Card inherits Rectangle {
        background: Theme.surface;
        border-color: Theme.border;
        border-width: 1px;
        border-radius: 12px;
    }

    component ActionButton inherits Rectangle {
        in property <string> label;
        in property <bool> primary;
        in property <bool> nav;
        in property <bool> selected;
        in property <bool> align-left;
        in property <bool> enabled: true;
        callback clicked;

        min-width: caption.preferred-width + 28px;
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

        caption := Text {
            x: 14px;
            width: parent.width - 28px;
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
                        background: project.archived ? Theme.muted : Theme.green;
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
        property <[string]> links: ["Heute", "Zeiten", "Projekte", "Berichte", "Einstellungen"];
        background: Theme.rail;

        if !root.compact: VerticalLayout {
            padding: 18px;
            spacing: 7px;
            Label { text: "Projektzeit"; color: Theme.rail-ink; font-size: 25px; font-weight: 720; }
            Rectangle { height: 12px; }
            for link[index] in root.links: ActionButton {
                label: link; nav: true; align-left: true; selected: index == root.page;
                clicked => { root.navigate(index); }
            }
            Rectangle { vertical-stretch: 1; }
            ActionButton {
                label: "Über Projektzeit"; nav: true; align-left: true; selected: root.page == 5;
                clicked => { root.navigate(5); }
            }
        }
        if root.compact: VerticalLayout {
            padding: 8px;
            spacing: 3px;
            HorizontalLayout {
                Label { text: "Projektzeit"; color: Theme.rail-ink; font-size: 19px; font-weight: 700; vertical-alignment: center; }
                Rectangle { horizontal-stretch: 1; }
                ActionButton { label: "Über"; nav: true; selected: root.page == 5; clicked => { root.navigate(5); } }
            }
            HorizontalLayout {
                spacing: 2px;
                for link[index] in root.links: ActionButton {
                    label: link; nav: true; selected: index == root.page;
                    horizontal-stretch: 1;
                    clicked => { root.navigate(index); }
                }
            }
        }
    }

    component WeekdayField inherits VerticalLayout {
        in property <string> label;
        in property <bool> editable: true;
        out property <string> amount: field.text;
        spacing: 4px;
        Label { text: root.label; }
        field := LineEdit {
            text: "00:00"; enabled: root.editable; accessible-label: root.label;
            min-width: 50px; preferred-width: 72px; horizontal-stretch: 1;
        }
    }

    component AccountSettings inherits VerticalLayout {
        in property <int> scheme;
        in property <string> date;
        in property <[string]> settings;
        in property <[string]> history;
        in property <string> preview-text;
        in property <bool> preview-ready;
        callback change-scheme(int);
        callback preview-edit(EditFields);
        callback confirm-edit();
        callback cancel-edit();
        spacing: 14px;
        alignment: start;

        Card {
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

        Card {
            VerticalLayout {
                padding: 16px;
                spacing: 10px;
                SectionTitle { text: "Arbeitszeitkonto"; }
                kind := ComboBox {
                    model: ["Wochenplan", "Startsaldo", "Tagessoll setzen", "Tagessoll zurücksetzen",
                            "Korrektur hinzufügen", "Korrektur bearbeiten", "Korrektur stornieren"];
                    current-index: 0;
                    accessible-label: "Art der Änderung";
                    enabled: !root.preview-ready;
                }
                HorizontalLayout {
                    spacing: 8px;
                    date-field := LineEdit {
                        enabled: !root.preview-ready && kind.current-index != 6;
                        placeholder-text: "Gültig ab (TT.MM.JJJJ)";
                        accessible-label: "Gültig ab";
                        text: root.date;
                        min-width: 105px;
                    }
                    amount := LineEdit {
                        enabled: !root.preview-ready && kind.current-index != 3 && kind.current-index != 6;
                        placeholder-text: kind.current-index == 0 ? "Wochensoll (hh:mm)" : "Dauer (±hh:mm)";
                        accessible-label: "Dauer";
                        min-width: 110px;
                    }
                }
                HorizontalLayout {
                    visible: kind.current-index == 0;
                    height: kind.current-index == 0 ? self.preferred-height : 0px;
                    spacing: 5px;
                    monday := WeekdayField { editable: !root.preview-ready; label: "Mo"; }
                    tuesday := WeekdayField { editable: !root.preview-ready; label: "Di"; }
                    wednesday := WeekdayField { editable: !root.preview-ready; label: "Mi"; }
                    thursday := WeekdayField { editable: !root.preview-ready; label: "Do"; }
                    friday := WeekdayField { editable: !root.preview-ready; label: "Fr"; }
                    saturday := WeekdayField { editable: !root.preview-ready; label: "Sa"; }
                    sunday := WeekdayField { editable: !root.preview-ready; label: "So"; }
                }
                HorizontalLayout {
                    spacing: 8px;
                    reason := LineEdit {
                        enabled: !root.preview-ready && kind.current-index >= 2;
                        placeholder-text: "Begründung";
                        accessible-label: "Begründung";
                        min-width: 90px;
                    }
                    entry := LineEdit {
                        enabled: !root.preview-ready && kind.current-index >= 5;
                        placeholder-text: "Korrektur-Nr.";
                        accessible-label: "Korrektur-Nr.";
                        min-width: 90px;
                    }
                    ActionButton {
                        label: "Vorschau"; primary: true; enabled: !root.preview-ready;
                        clicked => {
                            root.preview-edit({
                                kind: kind.current-index, date: date-field.text, amount: amount.text,
                                monday: monday.amount, tuesday: tuesday.amount, wednesday: wednesday.amount,
                                thursday: thursday.amount, friday: friday.amount, saturday: saturday.amount,
                                sunday: sunday.amount, reason: reason.text, entry-id: entry.text
                            });
                        }
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

        Card {
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
        in property <int> range-index;
        in property <string> heading;
        in property <string> subheading;
        in property <string> notice;
        in property <string> feedback;
        in property <string> report-date;
        in property <bool> has-report;
        in property <float> progress;
        in property <string> progress-label;
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
        callback step-period(int);
        callback go-to-date(string);
        callback refresh();
        callback preview-edit(EditFields);
        callback confirm-edit();
        callback cancel-edit();
        callback export-to(string);

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
                    width: 190px;
                    page: root.page;
                    navigate(index) => { root.navigate(index); }
                }
                VerticalLayout {
                    horizontal-stretch: 1;
                    padding: root.compact ? 14px : 28px;
                    spacing: 16px;
                    HorizontalLayout {
                        spacing: 10px;
                        VerticalLayout {
                            spacing: 3px;
                            Text { text: root.heading; color: Theme.ink; font-size: 31px; font-weight: 720; }
                            if root.subheading != "": Label { text: root.subheading; wrap: word-wrap; }
                        }
                        Rectangle { horizontal-stretch: 1; }
                        if root.page < 4: ActionButton { label: "Aktualisieren"; clicked => { root.refresh(); } }
                    }
                    if root.feedback != "": Label { text: root.feedback; color: Theme.green; wrap: word-wrap; }

                    if root.page == 1 || root.page == 3: VerticalLayout {
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

                            if root.page == 0 || ((root.page == 1 || root.page == 3) && root.has-report): StatCards {
                                items: root.cards;
                                narrow: root.narrow;
                                progress: root.page == 0 ? root.progress : 0.0;
                                progress-label: root.page == 0 ? root.progress-label : "";
                            }

                            if !root.has-report && (root.page == 0 || root.page == 1 || root.page == 3): Card {
                                background: Theme.tint;
                                VerticalLayout {
                                    padding: 16px;
                                    spacing: 8px;
                                    SectionTitle { text: "Arbeitszeitkonto einrichten"; }
                                    Label { text: root.notice; wrap: word-wrap; }
                                    HorizontalLayout {
                                        ActionButton { label: "Einstellungen öffnen"; primary: true; clicked => { root.navigate(4); } }
                                        Rectangle { horizontal-stretch: 1; }
                                    }
                                }
                            }
                            if root.has-report && root.notice != "": Label { text: root.notice; wrap: word-wrap; }

                            if root.page == 0 && root.has-report && !root.compact: HorizontalLayout {
                                spacing: 14px;
                                TimelinePanel { segments: root.segments; heading: "Tagesverlauf"; horizontal-stretch: 3; }
                                BalancePanel { metrics: root.metrics; heading: "Arbeitszeitkonto"; horizontal-stretch: 2; min-width: 260px; }
                            }
                            if root.page == 0 && (!root.has-report || root.compact): TimelinePanel { segments: root.segments; heading: "Tagesverlauf"; }
                            if root.page == 0 && root.has-report && root.compact: BalancePanel { metrics: root.metrics; heading: "Arbeitszeitkonto"; }

                            if root.page == 1 && root.range-index == 0: TimelinePanel { segments: root.segments; heading: "Tagesverlauf"; }
                            if root.page == 1 && root.rows.length > 0: Card {
                                VerticalLayout {
                                    padding: 16px;
                                    spacing: 4px;
                                    DayLine { day: { date: "Datum", actual: "Ist netto", target: "Soll", delta: "Differenz" }; header: true; }
                                    for day in root.rows: DayLine { day: day; }
                                }
                            }
                            if root.has-report && (root.page == 1 || root.page == 3): BalancePanel {
                                metrics: root.metrics;
                                heading: root.page == 1 ? "Zeitraumsaldo" : "Arbeitszeitkonto";
                            }
                            if root.has-report && (root.page == 1 || root.page == 3): Card {
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
                            if root.page == 4: AccountSettings {
                                scheme: root.scheme; date: root.report-date;
                                settings: root.settings; history: root.history;
                                preview-text: root.preview-text; preview-ready: root.preview-ready;
                                change-scheme(index) => { root.change-scheme(index); }
                                preview-edit(fields) => { root.preview-edit(fields); }
                                confirm-edit => { root.confirm-edit(); }
                                cancel-edit => { root.cancel-edit(); }
                            }
                            if root.page == 5: Card {
                                VerticalLayout { padding: 18px; AboutSlint {} }
                            }
                        }
                    }
                }
            }
        }
    }
}
