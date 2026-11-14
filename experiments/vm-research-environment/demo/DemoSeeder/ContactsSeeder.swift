import Contacts
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Replaces the dataset's people and groups in the Contacts store. Earlier copies with the
/// same names are deleted first, so the step can be repeated.
struct ContactsSeeder {
    let dataset: ContactsFile
    let portraits: [String: URL]
    let dates: RelativeDates
    /// Contacts whose birthday is placed relative to the seeding day: id → day offset.
    let relativeBirthdays: [String: Int]

    func run() async throws {
        let store = CNContactStore()
        guard try await store.requestAccess(for: .contacts) else {
            throw SeederError.denied("Contacts")
        }
        try removePrevious(from: store)

        let save = CNSaveRequest()
        var groups: [String: CNMutableGroup] = [:]
        for name in Set(dataset.contacts.flatMap(\.groups)).sorted() {
            let group = CNMutableGroup()
            group.name = name
            save.add(group, toContainerWithIdentifier: nil)
            groups[name] = group
        }
        for contact in dataset.contacts {
            let record = try makeRecord(contact)
            save.add(record, toContainerWithIdentifier: nil)
            for name in contact.groups {
                save.addMember(record, to: groups[name]!)
            }
        }
        try store.execute(save)
        log("contacts: \(dataset.contacts.count) people in \(groups.count) groups")
    }

    private func removePrevious(from store: CNContactStore) throws {
        let names = Set(dataset.contacts.map(\.displayName))
        let keys = [CNContactFormatter.descriptorForRequiredKeys(for: .fullName)]
        let save = CNSaveRequest()
        var removed = 0
        try store.enumerateContacts(with: CNContactFetchRequest(keysToFetch: keys)) { existing, _ in
            let name = CNContactFormatter.string(from: existing, style: .fullName) ?? existing.organizationName
            if names.contains(name) {
                save.delete(existing.mutableCopy() as! CNMutableContact)
                removed += 1
            }
        }
        let groupNames = Set(dataset.contacts.flatMap(\.groups))
        for group in try store.groups(matching: nil) where groupNames.contains(group.name) {
            save.delete(group.mutableCopy() as! CNMutableGroup)
        }
        try store.execute(save)
        if removed > 0 { log("contacts: removed \(removed) earlier copies") }
    }

    private func makeRecord(_ contact: Contact) throws -> CNMutableContact {
        let record = CNMutableContact()
        record.contactType = contact.isCompany ? .organization : .person
        record.givenName = contact.given ?? ""
        record.familyName = contact.family ?? ""
        record.organizationName = contact.organization ?? ""
        record.jobTitle = contact.jobTitle ?? ""
        record.phoneNumbers = contact.phones.map {
            CNLabeledValue(label: label($0.label), value: CNPhoneNumber(stringValue: $0.number))
        }
        record.emailAddresses = contact.emails.map {
            CNLabeledValue(label: label($0.label), value: $0.address as NSString)
        }
        if let address = contact.address {
            let postal = CNMutablePostalAddress()
            postal.street = address.street
            postal.city = address.city
            postal.state = address.state
            postal.postalCode = address.postalCode
            postal.country = address.country
            record.postalAddresses = [CNLabeledValue(label: label(address.label), value: postal)]
        }
        if let offset = relativeBirthdays[contact.id] {
            var birthday = Calendar.current.dateComponents([.month, .day], from: dates.day(offset))
            birthday.year = contact.birthday?.year
            record.birthday = birthday
        } else if let birthday = contact.birthday, let month = birthday.month, let day = birthday.day {
            record.birthday = DateComponents(year: birthday.year, month: month, day: day)
        }
        if contact.photo, let portrait = portraits[contact.id] {
            record.imageData = try thumbnailJPEG(of: portrait, maxPixels: 600)
            // Contacts and Messages draw avatars from the thumbnail, which the store derives
            // only for pictures set in Contacts itself, so a seeded contact supplies its own.
            record.setValue(try avatarJPEG(of: portrait, side: 320), forKey: "thumbnailImageData")
        }
        return record
    }

    private func label(_ name: String) -> String {
        switch name {
        case "mobile": CNLabelPhoneNumberMobile
        case "main": CNLabelPhoneNumberMain
        case "work": CNLabelWork
        default: CNLabelHome
        }
    }
}

func thumbnailJPEG(of url: URL, maxPixels: Int) throws -> Data {
    try jpeg(thumbnail(of: url, maxPixels: maxPixels), source: url)
}

/// A square crop for an avatar, centred across and nearer the top, where a portrait's face is.
func avatarJPEG(of url: URL, side: Int) throws -> Data {
    let image = try thumbnail(of: url, maxPixels: side * 2)
    let crop = min(image.width, image.height)
    let rect = CGRect(x: (image.width - crop) / 2, y: (image.height - crop) / 4, width: crop, height: crop)
    guard let square = image.cropping(to: rect) else { throw SeederError.unreadable(url) }
    return try jpeg(square, source: url)
}

private func thumbnail(of url: URL, maxPixels: Int) throws -> CGImage {
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
              kCGImageSourceCreateThumbnailFromImageAlways: true,
              kCGImageSourceThumbnailMaxPixelSize: maxPixels,
              kCGImageSourceCreateThumbnailWithTransform: true,
          ] as CFDictionary)
    else { throw SeederError.unreadable(url) }
    return image
}

private func jpeg(_ image: CGImage, source url: URL) throws -> Data {
    let data = NSMutableData()
    let destination = CGImageDestinationCreateWithData(data, UTType.jpeg.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
    guard CGImageDestinationFinalize(destination) else { throw SeederError.unreadable(url) }
    return data as Data
}
