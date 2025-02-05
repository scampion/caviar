# Caviar


<p align="center">
  <img src="logo.png"?raw=true" style="width: 200px; height: auto;" />
</p>

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
    ./target/release/caviar
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

This project is provided under the Commons Clause License Condition v1.0 (see [LICENSE](LICENSE) file for details) and follows the [Fair-code](https://faircode.io) principles.
The license allows free non-production use. For commercial use or production deployments, please contact the author to discuss licensing options.

## Author

Sébastien Campion - sebastien.campion@foss4.eu
