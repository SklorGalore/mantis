# Mantis Project Overview

Mantis is a power flow and linear sensitivity library written in Rust, designed to analyze electrical power system networks. It provides a web server with a comprehensive REST API for managing, modifying, and performing basic analysis on power system data. The project aims to offer a robust backend for power system simulations and data handling.

## Key Technologies

*   **Rust:** The core language for the library and server.
*   **Axum:** A web application framework for Rust, used to build the REST API.
*   **Tokio:** An asynchronous runtime for Rust, used for handling concurrent operations in the web server.
*   **Serde:** A powerful serialization/deserialization framework for Rust data structures, used for handling JSON data in the API.
*   **Nalgebra / Rsparse:** Libraries for numerical linear algebra, essential for power flow calculations.
*   **env_logger / log:** For logging within the application.
*   **tower-http:** Provides common HTTP utilities, including CORS configuration.

## Architecture

The project is structured as a Rust library (`src/lib.rs`) that exposes several modules:
*   `case`: Likely defines data structures for power system components (buses, branches, generators, loads, etc.).
*   `export`: Handles exporting network data, e.g., to industry-standard RAW format.
*   `loadflow`: Contains the logic for power flow calculations, such as DC approximation.
*   `parse`: Responsible for parsing industry-standard power flow case files (e.g., RAW format).
*   `server`: Implements the web server and REST API using Axum.

The main entry point (`src/main.rs`) simply starts the web server, which listens on port `3000`. The server maintains the state of the power system `Network` in memory, protected by `Arc<Mutex<Option<Network>>>` for thread-safe access.

## Building and Running

To build and run the Mantis project:

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/SklorGalore/mantis.git
    cd mantis
    ```
2.  **Build the project:**
    ```bash
    cargo build
    ```
3.  **Run the server:**
    ```bash
    cargo run
    ```
    The server will start and be accessible at `http://localhost:3000`.

## Development Conventions

*   **Language:** Rust
*   **Asynchronous Programming:** Uses `tokio` for async operations.
*   **API Framework:** `axum` for building the web API.
*   **State Management:** The power system network state is managed in-memory within the server using `Arc<Mutex<Option<Network>>>`.
*   **Data Serialization:** `serde` is used for JSON serialization and deserialization for API communication.
*   **Logging:** `env_logger` and `log` are used for logging.
*   **Static Assets:** The server serves `static/index.html` and `static/mantis16.png`.

## API Endpoints

The server exposes the following REST API endpoints:

### General
*   `GET /`: Serves the `index.html` file.
*   `GET /logo.png`: Serves the `mantis16.png` logo.

### Network Management
*   `POST /api/upload`: Uploads a RAW format power flow case file (multipart/form-data with field "file"). Parses the file and loads the network.
*   `GET /api/network`: Retrieves the currently loaded power system network.
*   `PUT /api/network`: Replaces the entire network with the provided JSON representation.
*   `POST /api/network/new`: Creates a new, empty network. Requires `case_name`, `s_base`, and `frequency` in the JSON body.

### Component Management (CRUD operations for individual components)

**Buses (`/api/buses`)**
*   `POST /api/buses`: Adds a new bus.
*   `PUT /api/buses/{id}`: Updates an existing bus by ID.
*   `DELETE /api/buses/{id}`: Deletes a bus by ID, cascading to connected branches, loads, and generators.

**Branches (`/api/branches`)**
*   `POST /api/branches`: Adds a new branch.
*   `PUT /api/branches/{id}`: Updates an existing branch by ID.
*   `DELETE /api/branches/{id}`: Deletes a branch by ID.

**Generators (`/api/generators`)**
*   `POST /api/generators`: Adds a new generator.
*   `PUT /api/generators/{id}`: Updates an existing generator by ID.
*   `DELETE /api/generators/{id}`: Deletes a generator by ID.

**Loads (`/api/loads`)**
*   `POST /api/loads`: Adds a new load.
*   `PUT /api/loads/{id}`: Updates an existing load by ID.
*   `DELETE /api/loads/{id}`: Deletes a load by ID.

**Fixed Shunts (`/api/fixed-shunts`)**
*   `POST /api/fixed-shunts`: Adds a new fixed shunt.
*   `PUT /api/fixed-shunts/{idx}`: Updates an existing fixed shunt by index.
*   `DELETE /api/fixed-shunts/{idx}`: Deletes a fixed shunt by index.

**Switched Shunts (`/api/switched-shunts`)**
*   `POST /api/switched-shunts`: Adds a new switched shunt.
*   `PUT /api/switched-shunts/{idx}`: Updates an existing switched shunt by index.
*   `DELETE /api/switched-shunts/{idx}`: Deletes a switched shunt by index.

**Areas (`/api/areas`)**
*   `POST /api/areas`: Adds a new area.
*   `PUT /api/areas/{id}`: Updates an existing area by ID.
*   `DELETE /api/areas/{id}`: Deletes an area by ID.

**Zones (`/api/zones`)**
*   `POST /api/zones`: Adds a new zone.
*   `PUT /api/zones/{id}`: Updates an existing zone by ID.
*   `DELETE /api/zones/{id}`: Deletes a zone by ID.

### Analysis
*   `POST /api/solve/dc`: Performs a DC power flow approximation on the loaded network.

### Export
*   `GET /api/export`: Exports the current network as a RAW file.
