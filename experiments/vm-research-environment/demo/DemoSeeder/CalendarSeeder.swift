import EventKit
import Foundation

/// Creates the dataset's calendars in the local ("On My Mac") source and fills them.
/// Calendars with the same titles are deleted first, so the step can be repeated.
struct CalendarSeeder {
    let dataset: CalendarFile
    let dates: RelativeDates

    func run() async throws {
        let store = EKEventStore()
        guard try await store.requestFullAccessToEvents() else { throw SeederError.denied("Calendars") }
        let source = try localSource(in: store)

        var calendars: [String: EKCalendar] = [:]
        for spec in dataset.calendars {
            for existing in store.calendars(for: .event) where existing.title == spec.title {
                try store.removeCalendar(existing, commit: false)
            }
            let calendar = EKCalendar(for: .event, eventStore: store)
            calendar.title = spec.title
            calendar.cgColor = .hex(spec.color)
            calendar.source = source
            try store.saveCalendar(calendar, commit: false)
            calendars[spec.title] = calendar
        }
        try store.commit()

        for spec in dataset.events {
            guard let calendar = calendars[spec.calendar] else {
                throw SeederError.invalid("event '\(spec.title)' names unknown calendar '\(spec.calendar)'")
            }
            try store.save(makeEvent(spec, in: calendar, store: store), span: .futureEvents, commit: false)
        }
        try store.commit()
        log("calendar: \(dataset.events.count) events in \(calendars.count) calendars")
    }

    private func makeEvent(_ spec: CalendarFile.Event, in calendar: EKCalendar, store: EKEventStore) -> EKEvent {
        let event = EKEvent(eventStore: store)
        event.calendar = calendar
        event.title = spec.title
        event.location = spec.location
        event.notes = spec.notes
        event.url = spec.url.flatMap(URL.init(string:))
        let day = firstOccurrence(of: spec)
        if spec.allDay {
            event.isAllDay = true
            event.startDate = dates.day(day)
            event.endDate = dates.day(day + max(spec.days ?? 1, 1) - 1)
        } else {
            event.startDate = dates.date(day: day, time: spec.start)
            event.endDate = dates.date(day: day, time: spec.end ?? spec.start)
        }
        if let minutes = spec.alarmMinutes {
            event.addAlarm(EKAlarm(relativeOffset: TimeInterval(-minutes * 60)))
        }
        if let recurrence = spec.recurrence {
            event.addRecurrenceRule(rule(recurrence))
        }
        return event
    }

    /// A weekly rule's first occurrence is moved onto its first matching weekday on or
    /// after the event's day, so the dataset need not know which weekday day 0 is.
    private func firstOccurrence(of spec: CalendarFile.Event) -> Int {
        guard let weekdays = spec.recurrence?.daysOfWeek, !weekdays.isEmpty else { return spec.day }
        let wanted = Set(weekdays.map(weekdayNumber))
        return (spec.day..<spec.day + 7).first {
            wanted.contains(dates.calendar.component(.weekday, from: dates.day($0)))
        }!
    }

    private func rule(_ recurrence: CalendarFile.Recurrence) -> EKRecurrenceRule {
        let frequency: EKRecurrenceFrequency = switch recurrence.frequency {
        case "monthly": .monthly
        case "yearly": .yearly
        default: .weekly
        }
        let days = recurrence.daysOfWeek?.map { EKRecurrenceDayOfWeek(EKWeekday(rawValue: weekdayNumber($0))!) }
        let end = recurrence.untilDay.map { EKRecurrenceEnd(end: dates.day($0)) }
        return EKRecurrenceRule(
            recurrenceWith: frequency, interval: recurrence.interval ?? 1, daysOfTheWeek: days,
            daysOfTheMonth: nil, monthsOfTheYear: nil, weeksOfTheYear: nil, daysOfTheYear: nil,
            setPositions: nil, end: end
        )
    }

    private func weekdayNumber(_ name: String) -> Int {
        ["sun", "mon", "tue", "wed", "thu", "fri", "sat"].firstIndex(of: String(name.prefix(3)).lowercased())! + 1
    }
}

/// The "On My Mac" source, which exists on a Mac without iCloud.
func localSource(in store: EKEventStore) throws -> EKSource {
    if let local = store.sources.first(where: { $0.sourceType == .local }) { return local }
    if let fallback = store.defaultCalendarForNewEvents?.source { return fallback }
    throw SeederError.invalid("no local calendar source; sources: \(store.sources.map(\.title))")
}
