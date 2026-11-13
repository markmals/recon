import AVFoundation
import Foundation
import ImageIO
import Photos

/// Imports the demo photos and videos into the system Photo Library. Capture dates keep
/// their spacing but are shifted so the newest item falls on the evening before the
/// seeding day, so the library reads as recent; GPS metadata is kept. Shoots named in the
/// file names become albums.
struct PhotosSeeder {
    let media: [URL]
    let dates: RelativeDates

    /// File-name tag → album title.
    static let albums: [(tag: String, title: String)] = [
        ("demo-la", "Los Angeles"), ("sicily", "Sicily"), ("tahoe", "Lake Tahoe"),
        ("valleyoffire", "Valley of Fire"), ("mexicocity", "Mexico City"), ("india", "India"),
        ("kanab", "Kanab"), ("amalfi", "Amalfi Coast"), ("marin", "Marin Headlands"),
        ("socalroadtrip", "SoCal Road Trip"),
    ]

    func run() async throws {
        let status = await PHPhotoLibrary.requestAuthorization(for: .readWrite)
        guard status == .authorized else { throw SeederError.denied("Photos (status \(status.rawValue))") }
        let existing = PHAsset.fetchAssets(with: nil).count
        guard existing == 0 else {
            throw SeederError.invalid("the Photo Library already holds \(existing) items; seed photos into an empty library")
        }

        var items: [(url: URL, date: Date, isVideo: Bool)] = []
        for url in media {
            let isVideo = ["mov", "m4v", "mp4"].contains(url.pathExtension.lowercased())
            items.append((url, try await captureDate(of: url, isVideo: isVideo), isVideo))
        }
        guard let newest = items.map(\.date).max() else { return }
        let shift = dates.date(day: -1, time: "17:30").timeIntervalSince(newest)

        let albums = try await createAlbums()
        for (index, batch) in items.chunked(by: 20).enumerated() {
            try await PHPhotoLibrary.shared().performChanges {
                for (offset, item) in batch.enumerated() {
                    let request = PHAssetCreationRequest.forAsset()
                    request.addResource(with: item.isVideo ? .video : .photo, fileURL: item.url, options: nil)
                    request.creationDate = item.date.addingTimeInterval(shift)
                    request.isFavorite = (index * 20 + offset) % 13 == 0
                    let name = item.url.lastPathComponent.lowercased()
                    for (tag, title) in Self.albums where name.contains(tag) {
                        guard let album = albums[title], let placeholder = request.placeholderForCreatedAsset else { continue }
                        PHAssetCollectionChangeRequest(for: album)?.addAssets([placeholder] as NSArray)
                    }
                }
            }
            log("photos: imported \(min((index + 1) * 20, items.count)) of \(items.count)")
        }
    }

    private func createAlbums() async throws -> [String: PHAssetCollection] {
        var identifiers: [String: String] = [:]
        try await PHPhotoLibrary.shared().performChanges {
            for (_, title) in Self.albums {
                identifiers[title] = PHAssetCollectionChangeRequest
                    .creationRequestForAssetCollection(withTitle: title)
                    .placeholderForCreatedAssetCollection.localIdentifier
            }
        }
        let fetched = PHAssetCollection.fetchAssetCollections(withLocalIdentifiers: Array(identifiers.values), options: nil)
        var albums: [String: PHAssetCollection] = [:]
        fetched.enumerateObjects { collection, _, _ in albums[collection.localizedTitle ?? ""] = collection }
        return albums
    }

    private func captureDate(of url: URL, isVideo: Bool) async throws -> Date {
        if isVideo, let date = try await AVURLAsset(url: url).load(.creationDate)?.load(.dateValue) {
            return date
        }
        if !isVideo,
           let source = CGImageSourceCreateWithURL(url as CFURL, nil),
           let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
           let exif = properties[kCGImagePropertyExifDictionary] as? [CFString: Any],
           let original = exif[kCGImagePropertyExifDateTimeOriginal] as? String,
           let date = Self.exifFormatter.date(from: original) {
            return date
        }
        return try url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate ?? Date()
    }

    private static let exifFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy:MM:dd HH:mm:ss"
        return formatter
    }()
}

extension Array {
    func chunked(by size: Int) -> [ArraySlice<Element>] {
        stride(from: 0, to: count, by: size).map { self[$0..<Swift.min($0 + size, count)] }
    }
}
