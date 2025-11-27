import SwiftUI

struct ContentView: View {
    @EnvironmentObject var dbManager: DatabaseManager
    @State private var showingAddPerson = false
    @State private var searchCity = ""
    @State private var isSearching = false

    var body: some View {
        NavigationView {
            VStack {
                // Search bar
                HStack {
                    TextField("Search by city", text: $searchCity)
                        .textFieldStyle(RoundedBorderTextFieldStyle())
                        .autocapitalization(.words)

                    if isSearching {
                        Button("Clear") {
                            isSearching = false
                            searchCity = ""
                            Task {
                                await dbManager.loadPeople()
                            }
                        }
                        .buttonStyle(.bordered)
                    } else {
                        Button("Search") {
                            isSearching = true
                            Task {
                                await dbManager.searchPeople(city: searchCity)
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(searchCity.isEmpty)
                    }
                }
                .padding()

                // Error message
                if let error = dbManager.errorMessage {
                    Text(error)
                        .foregroundColor(.red)
                        .font(.caption)
                        .padding(.horizontal)
                }

                // People list
                if dbManager.isLoading {
                    ProgressView("Loading...")
                        .padding()
                } else if dbManager.people.isEmpty {
                    VStack(spacing: 16) {
                        Image(systemName: "person.3.fill")
                            .font(.system(size: 60))
                            .foregroundColor(.gray)

                        Text(isSearching ? "No people found in \(searchCity)" : "No people yet")
                            .font(.headline)
                            .foregroundColor(.gray)

                        Text(isSearching ? "Try a different city" : "Tap + to add your first person")
                            .font(.subheadline)
                            .foregroundColor(.gray)
                    }
                    .padding()
                } else {
                    List {
                        ForEach(dbManager.people) { person in
                            PersonRow(person: person)
                        }
                        .onDelete { indexSet in
                            Task {
                                for index in indexSet {
                                    await dbManager.deletePerson(name: dbManager.people[index].name)
                                }
                            }
                        }
                    }
                }

                Spacer()
            }
            .navigationTitle("GraphLite Demo")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Menu {
                        Button(role: .destructive, action: {
                            Task {
                                await dbManager.clearDatabase()
                            }
                        }) {
                            Label("Clear All", systemImage: "trash")
                        }

                        Button(action: {
                            Task {
                                await dbManager.loadPeople()
                            }
                        }) {
                            Label("Refresh", systemImage: "arrow.clockwise")
                        }

                        Button(action: loadSampleData) {
                            Label("Load Sample Data", systemImage: "person.3")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }

                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        showingAddPerson = true
                    } label: {
                        Image(systemName: "plus")
                    }
                }
            }
            .sheet(isPresented: $showingAddPerson) {
                AddPersonView()
                    .environmentObject(dbManager)
            }
        }
    }

    private func loadSampleData() {
        Task {
            await dbManager.addPerson(name: "Alice Johnson", age: 30, city: "New York")
            await dbManager.addPerson(name: "Bob Smith", age: 25, city: "San Francisco")
            await dbManager.addPerson(name: "Carol Williams", age: 28, city: "Los Angeles")
            await dbManager.addPerson(name: "Dave Brown", age: 32, city: "New York")
            await dbManager.addPerson(name: "Eve Davis", age: 27, city: "Chicago")
        }
    }
}

struct PersonRow: View {
    let person: Person

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(person.name)
                .font(.headline)

            HStack {
                Label("\(person.age)", systemImage: "birthday.cake")
                    .font(.caption)

                Spacer()

                Label(person.city, systemImage: "location")
                    .font(.caption)
            }
            .foregroundColor(.secondary)
        }
        .padding(.vertical, 4)
    }
}

struct AddPersonView: View {
    @EnvironmentObject var dbManager: DatabaseManager
    @Environment(\.dismiss) var dismiss

    @State private var name = ""
    @State private var age = 25
    @State private var city = ""

    var body: some View {
        NavigationView {
            Form {
                Section(header: Text("Person Information")) {
                    TextField("Name", text: $name)
                        .autocapitalization(.words)

                    Stepper("Age: \(age)", value: $age, in: 1...120)

                    TextField("City", text: $city)
                        .autocapitalization(.words)
                }
            }
            .navigationTitle("Add Person")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") {
                        dismiss()
                    }
                }

                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Add") {
                        Task {
                            await dbManager.addPerson(name: name, age: age, city: city)
                            dismiss()
                        }
                    }
                    .disabled(name.isEmpty || city.isEmpty)
                }
            }
        }
    }
}

#Preview {
    ContentView()
        .environmentObject(DatabaseManager())
}
