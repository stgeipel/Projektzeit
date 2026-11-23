slint::slint! {
    #[edition = "1.19"]
    import { AboutSlint, Button, ComboBox, LineEdit, ScrollView, VerticalBox } from "std-widgets.slint";

    export struct Metric { label: string, value: string }
    export struct DayRow { date: string, actual: string, target: string, delta: string }
    export struct EditFields {
        kind: int,
        date: string,
        amount: string,
        monday: string,
        tuesday: string,
        wednesday: string,
        thursday: string,
        friday: string,
        saturday: string,
        sunday: string,
        reason: string,
        entry-id: string,
    }

    component WeekdayField inherits VerticalLayout {
        in property <string> label;
        in property <bool> editable: true;
        out property <string> amount: field.text;
        spacing: 3px;
        Text { text: root.label; color: #234439; }
        field := LineEdit {
            text: "00:00";
            enabled: root.editable;
            min-width: 72px;
            preferred-width: 96px;
            horizontal-stretch: 1;
        }
    }

    component DayLine inherits Rectangle {
        in property <DayRow> day;
        in property <bool> header;
        height: 27px;
        background: root.header ? #d9e7da : #f0f1e7;
        HorizontalLayout {
            padding-left: 10px;
            padding-right: 10px;
            Text { text: root.day.date; color: #234439; width: 175px; vertical-alignment: center; }
            Text { text: root.day.actual; color: #234439; horizontal-stretch: 1; horizontal-alignment: right; vertical-alignment: center; }
            Text { text: root.day.target; color: #234439; horizontal-stretch: 1; horizontal-alignment: right; vertical-alignment: center; }
            Text { text: root.day.delta; color: #234439; horizontal-stretch: 1; horizontal-alignment: right; vertical-alignment: center; }
        }
    }
    export component AccountWindow inherits Window {
        title: "Projektzeit · Arbeitszeitkonto";
        preferred-width: 920px;
        preferred-height: 780px;
        min-width: 780px;
        min-height: 600px;
        background: #f6f6ed;
        default-font-size: 15px;

        in property <int> page;
        in property <string> heading;
        in property <string> notice;
        in property <string> feedback;
        in property <string> report-date;
        in property <bool> has-report;
        in property <bool> exportable;
        in property <[Metric]> metrics;
        in property <[DayRow]> rows;
        in property <[string]> settings;
        in property <[string]> history;
        in property <string> preview-text;
        in property <bool> preview-ready;

        callback navigate(int);
        callback step-period(int);
        callback go-to-date(string);
        callback refresh();
        callback preview-edit(EditFields);
        callback confirm-edit();
        callback cancel-edit();
        callback export-to(string);

        VerticalLayout {
            padding: 22px;
            spacing: 12px;

            Text {
                text: "Arbeitszeitkonto";
                font-size: 30px;
                font-weight: 700;
                color: #173e32;
            }
            HorizontalLayout {
                spacing: 8px;
                Button { text: "Heute"; primary: root.page == 0; clicked => { root.navigate(0); } }
                Button { text: "Woche"; primary: root.page == 1; clicked => { root.navigate(1); } }
                Button { text: "Monat"; primary: root.page == 2; clicked => { root.navigate(2); } }
                Button { text: "Einstellungen"; primary: root.page == 3; clicked => { root.navigate(3); } }
                Rectangle { horizontal-stretch: 1; }
                Button { text: "Über"; primary: root.page == 4; clicked => { root.navigate(4); } }
            }

            if root.feedback != "": Text {
                text: root.feedback;
                color: #743d18;
                wrap: word-wrap;
            }

            if root.page < 3: VerticalLayout {
                spacing: 10px;
                if root.page > 0: HorizontalLayout {
                    spacing: 8px;
                    Button { text: "Zurück"; clicked => { root.step-period(-1); } }
                    Button { text: "Weiter"; clicked => { root.step-period(1); } }
                    Text { text: "Stichtag"; color: #234439; vertical-alignment: center; }
                    report-date-field := LineEdit {
                        text: root.report-date;
                        placeholder-text: "TT.MM.JJJJ";
                        preferred-width: 130px;
                        horizontal-stretch: 0;
                        accepted => { root.go-to-date(self.text); }
                    }
                    Button { text: "Anzeigen"; clicked => { root.go-to-date(report-date-field.text); } }
                    Rectangle { horizontal-stretch: 1; }
                    Button { text: "Aktualisieren"; clicked => { root.refresh(); } }
                }
                if root.page == 0: HorizontalLayout {
                    Rectangle { horizontal-stretch: 1; }
                    Button { text: "Aktualisieren"; clicked => { root.refresh(); } }
                }
                Text { text: root.heading; color: #173e32; font-size: 21px; font-weight: 700; }
                if root.notice != "": Text { text: root.notice; color: #314840; wrap: word-wrap; }
                if root.has-report: ScrollView {
                    content-width: self.visible-width;
                    VerticalLayout {
                        spacing: 6px;
                        for metric in root.metrics: Rectangle {
                            height: 28px;
                            background: #e7efe3;
                            HorizontalLayout {
                                padding-left: 10px;
                                padding-right: 10px;
                                Text { text: metric.label; color: #234439; vertical-alignment: center; }
                                Text { text: metric.value; color: #173e32; font-weight: 700; vertical-alignment: center; horizontal-alignment: right; }
                            }
                        }
                        if root.rows.length > 0: DayLine {
                            day: { date: "Datum", actual: "Ist netto", target: "Soll", delta: "Differenz" };
                            header: true;
                        }
                        for value in root.rows: DayLine { day: value; }
                    }
                }
                if !root.has-report: Rectangle { vertical-stretch: 1; }
                if root.exportable: HorizontalLayout {
                    spacing: 8px;
                    Text { text: "XLSX-Datei"; color: #234439; vertical-alignment: center; }
                    export-file := LineEdit { placeholder-text: "Dateipfad, z. B. Arbeitszeitkonto.xlsx"; }
                    Button { text: "Exportieren"; clicked => { root.export-to(export-file.text); } }
                }
            }

            if root.page == 3: ScrollView {
                content-width: self.visible-width;
                VerticalLayout {
                    spacing: 9px;
                    alignment: start;
                    Text { text: "Einstellungen"; color: #173e32; font-size: 21px; font-weight: 700; }
                    VerticalLayout {
                        spacing: 8px;
                        HorizontalLayout {
                            spacing: 8px;
                            kind := ComboBox {
                                model: ["Wochenplan", "Startsaldo", "Tagessoll setzen", "Tagessoll zurücksetzen",
                                        "Korrektur hinzufügen", "Korrektur bearbeiten", "Korrektur stornieren"];
                                current-index: 0;
                                enabled: !root.preview-ready;
                            }
                            date-field := LineEdit {
                                enabled: !root.preview-ready && kind.current-index != 6;
                                placeholder-text: "Gültig ab / Datum (TT.MM.JJJJ)";
                                text: root.report-date;
                            }
                            amount := LineEdit {
                                enabled: !root.preview-ready && kind.current-index != 3 && kind.current-index != 6;
                                placeholder-text: kind.current-index == 0 ? "Wochensoll (hh:mm)" : "Dauer (±hh:mm)";
                            }
                        }
                        HorizontalLayout {
                            visible: kind.current-index == 0;
                            spacing: 6px;
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
                            }
                            entry := LineEdit {
                                enabled: !root.preview-ready && kind.current-index >= 5;
                                placeholder-text: "Korrektur-Nr.";
                                preferred-width: 140px;
                                horizontal-stretch: 0;
                            }
                            Button {
                                text: "Vorschau";
                                enabled: !root.preview-ready;
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
                    }
                    if root.preview-ready: VerticalBox {
                        Text { text: root.preview-text; wrap: word-wrap; color: #173e32; }
                        HorizontalLayout {
                            spacing: 8px;
                            Button { text: "Abbrechen"; clicked => { root.cancel-edit(); } }
                            Button { text: "Änderung speichern"; clicked => { root.confirm-edit(); } }
                        }
                    }
                    Text { text: "Gespeicherte Einstellungen"; color: #173e32; font-weight: 700; }
                    for item in root.settings: Text { text: item; wrap: word-wrap; color: #234439; }
                    Text { text: "Änderungsverlauf"; color: #173e32; font-weight: 700; }
                    for event in root.history: Text { text: event; wrap: word-wrap; color: #234439; }
                }
            }

            if root.page == 4: VerticalLayout {
                spacing: 12px;
                Text { text: "Über Projektzeit"; color: #173e32; font-size: 21px; font-weight: 700; }
                AboutSlint {}
                Rectangle { vertical-stretch: 1; }
            }
        }
    }

}
