# GraphLite iOS Demo App

A complete iOS SwiftUI application demonstrating GraphLite usage on iOS.

## Features

- Add and manage people in a GraphLite database
- Search by city
- View list of people with ages and locations
- Delete people with swipe gesture
- Load sample data
- Clear entire database
- Persistent storage in iOS Documents directory

## Screenshots

The app includes:
- **Main List View**: Browse all people in the database
- **Search**: Filter people by city
- **Add Person**: Form to add new people
- **Actions Menu**: Refresh, clear, load sample data

## What it demonstrates

### iOS-Specific Features
- SwiftUI integration with `@EnvironmentObject`
- iOS file system (Documents directory)
- iOS UI components (NavigationView, List, Form, Sheets)
- Async/await with `@MainActor`
- Swipe-to-delete
- Pull-to-refresh
- Error handling with user feedback

### GraphLite Features
- Database initialization on iOS
- Session management
- Schema and graph creation with `IF NOT EXISTS`
- INSERT operations
- MATCH queries with WHERE clauses
- DELETE operations
- ORDER BY for sorting results

## Building and Running

### Prerequisites

1. **Build GraphLite FFI for iOS**:
   ```bash
   cd bindings/swift
   ./build-ios.sh
   ```

   This creates `GraphLiteFFI.xcframework` with iOS support.

2. **Xcode 15+** with iOS 13+ SDK

### Option 1: Run in Xcode

1. Create a new iOS App project in Xcode
2. Copy the source files into your project:
   - `GraphLiteDemoApp.swift`
   - `DatabaseManager.swift`
   - `ContentView.swift`

3. Add GraphLite Swift package dependency:
   - File → Add Package Dependencies
   - Add local package: `bindings/swift`
   - Or use repository URL: `https://github.com/GraphLite-AI/GraphLite`

4. Select iOS Simulator or device target
5. Build and run (Cmd+R)

### Option 2: Using SPM (Advanced)

Add an executable target to `Package.swift`:

```swift
.target(
    name: "GraphLiteDemoApp",
    dependencies: ["GraphLite"],
    path: "Examples/iOS/GraphLiteDemoApp"
)
```

Then:
```bash
swift build
# Note: Cannot run iOS apps directly via SPM, must use Xcode
```

## Project Structure

```
GraphLiteDemoApp/
 GraphLiteDemoApp.swift    # App entry point
 ContentView.swift          # Main UI (list, search, add)
 DatabaseManager.swift      # GraphLite database logic
 README.md                  # This file
```

## Code Architecture

### DatabaseManager

Observable class managing GraphLite operations:

```swift
@MainActor
class DatabaseManager: ObservableObject {
    @Published var people: [Person] = []
    @Published var isLoading = false
    @Published var errorMessage: String?

    private var db: GraphLite?
    private var session: Session?
}
```

Methods:
- `loadPeople()` - Query all people
- `addPerson()` - Insert new person
- `deletePerson()` - Delete by name
- `searchPeople()` - Filter by city
- `clearDatabase()` - Delete all nodes

### ContentView

Main UI with:
- NavigationView for app navigation
- Search bar for city filtering
- List with swipe-to-delete
- Add person sheet
- Actions menu (clear, refresh, sample data)

### Person Model

Simple struct for displaying data:
```swift
struct Person: Identifiable {
    let id: UUID
    let name: String
    let age: Int
    let city: String
}
```

## Database Schema

**Schema**: `/app`
**Graph**: `/app/people`

**Node Label**: `Person`

**Properties**:
- `name`: String
- `age`: Integer
- `city`: String

## Usage Flow

1. **First Launch**:
   - App creates database in Documents directory
   - Creates schema `/app` and graph `/app/people`
   - Shows empty state

2. **Add People**:
   - Tap + button
   - Fill form (name, age, city)
   - Tap "Add"
   - GraphLite executes INSERT query
   - List refreshes automatically

3. **Search**:
   - Enter city name in search field
   - Tap "Search"
   - GraphLite executes MATCH with WHERE clause
   - Results filtered by city

4. **Delete**:
   - Swipe left on person
   - Tap "Delete"
   - GraphLite executes DELETE query
   - List updates

5. **Load Sample Data**:
   - Tap menu (⋯)
   - Tap "Load Sample Data"
   - Adds 5 example people

## Testing on Simulator

```bash
# Build for iOS Simulator
cd bindings/swift
./build-ios.sh

# Open in Xcode
open Examples/iOS/GraphLiteDemoApp/

# Or use xcodebuild
xcodebuild -scheme GraphLiteDemoApp -destination 'platform=iOS Simulator,name=iPhone 15'
```

## Testing on Physical Device

1. Connect iPhone/iPad via USB or WiFi
2. Select device in Xcode
3. Click Run
4. Accept code signing (may need Apple Developer account)

## Database Location

The database is stored at:
```
/var/mobile/Containers/Data/Application/<UUID>/Documents/graphlite.db/
```

You can inspect it using Xcode's Device window:
- Window → Devices and Simulators
- Select device
- Select app
- View container → Download container
- Browse `Documents/graphlite.db/`

## Troubleshooting

### Build Errors

**Error: "No such module 'GraphLite'"**

Build the GraphLite package:
```bash
cd bindings/swift
swift build
```

**Error: "library not found for -lgraphlit_ffi"**

Build the iOS FFI:
```bash
cd bindings/swift
./build-ios.sh
```

### Runtime Errors

**Error: "Failed to setup database"**

Check that:
- App has write permissions (should be automatic for Documents directory)
- iOS version is 13+ (required for file system access)
- XCFramework includes iOS architecture

**Error: "Query execution failed"**

Check:
- GQL syntax is correct
- Schema and graph are set correctly
- Session is not closed

## Performance Notes

- Database is persistent across app launches
- Queries execute synchronously (wrapped in async/await)
- List updates happen on main thread (@MainActor)
- Typical query time: < 10ms for small datasets

## Next Steps

Enhance the demo to add:
- Relationships between people (friendships, follows)
- Relationship visualization
- Graph traversal queries
- Data export/import
- Statistics dashboard
- Dark mode support
- iPad optimization

## References

- [GraphLite Swift Bindings README](../../README.md)
- [Swift Package Manager](https://swift.org/package-manager/)
- [SwiftUI Documentation](https://developer.apple.com/xcode/swiftui/)
