# Environment Variables to JSON

A command-line utility written in Rust that converts environment variables into JSON format based on [JSON Schema](https://json-schema.org/).

## Why

Let's say you have a JSON Schema file that defines the structure of a configuration file. You want to use that configuration file in a program, but you don't want to write code to parse it. You can use this tool to convert the environment variables into a JSON object that matches the schema.

## Example

Using the basic schema from [JSON Schema Examples](https://json-schema.org/learn/miscellaneous-examples#basic):
```json
{
  "$id": "https://example.com/person.schema.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Person",
  "type": "object",
  "properties": {
    "first_name": {
      "type": "string",
      "description": "The person's first name."
    },
    "last_name": {
      "type": "string",
      "description": "The person's last name."
    },
    "age": {
      "description": "Age in years which must be equal to or greater than zero.",
      "type": "integer",
      "minimum": 0
    }
  }
}
```

And the following environment variables:
```bash
PERSON_FIRST_NAME=John
PERSON_LAST_NAME=Doe
PERSON_AGE=30
```

Running the following command:
```bash
cat example/basic-schema.json | env-to-schema-json --prefix PERSON_
```

The following JSON will be generated:
```json
{
  "first_name": "John",
  "last_name": "Doe",
  "age": 30
}
```

## Installation

## Usage

```bash
cat schema.json | env-to-schema-json --prefix <prefix>
# or
env-to-schema-json --prefix <prefix> < schema.json
```

## Development

Make sure you have Rust installed on your system. Then:

```bash
cargo run
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.
