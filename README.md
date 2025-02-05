# PII Detection Service

This is a service for detecting Personally Identifiable Information (PII) in text.

## Getting Started

To run the service, you will need to have Rust and Cargo installed.

1.  Clone the repository:

    ```bash
    git clone <repository_url>
    cd <repository_directory>
    ```

2.  Build the project:

    ```bash
    cargo build --release
    ```

3.  Run the service:

    ```bash
    ./target/release/pii-detection-service
    ```

The service will be available on port 8080.

## Usage

Send a POST request to `/detect_pii` with a JSON payload containing the text to analyze:

```json
{
  "text": "Example text containing a name like John Doe or an email address like john.doe@example.com."
}
```

The service will return a JSON response containing the entities detected:

```json
{
  "entities": [
    {
      "word": "John Doe",
      "entity": "PERSON",
      "score": 0.95,
      "start": 30,
      "end": 38,
      "index": 5
    },
    {
      "word": "john.doe@example.com",
      "entity": "EMAIL",
      "score": 0.98,
      "start": 54,
      "end": 75,
      "index": 9
    }
  ]
}
```

## Configuration

The service can be configured using environment variables. The following variables are supported:

*   `PORT`: The port to listen on (default: 8080).

## License

This project is licensed under the MIT License.
