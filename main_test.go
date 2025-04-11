package main

import (
	"encoding/json" // Import encoding/json for test output formatting
	"reflect"       // Import reflect package for DeepEqual
	"testing"
)

func TestProcessInput(t *testing.T) {
	// Prepare input JSON as bytes - Added non-matching XPath and custom tag test
	inputJsonBytes := []byte(`{
		"xpaths": ["/html/body/p", "//title", "/html/body/a/@href", "//nonexistent", "//jsLiteral"],
		"urls": {
			"http://example.com": {
				"content": "<html><head><title>Test Page</title></head><body><p>Hello World</p><a href=\"/link\">Click</a></body></html>"
			},
			"http://malformed.com": {
				"content": "<ht<ml>><body>Invalid"
			},
			"http://empty.com": {
				"content": ""
			},
			"http://customtag.com": {
				"content": "<doc><jsLiteral>some data</jsLiteral><other>stuff</other></doc>"
			}
		}
	}`)

	// Prepare expected output structure
	// Prepare expected output structure - Malformed/Empty URLs are now omitted
	expectedOutput := OutputJson{
		"/html/body/p": {
			"http://example.com": "Hello World",
			// "http://malformed.com" is omitted due to parsing error
			// "http://empty.com" is omitted due to parsing error (EOF)
		},
		"//title": {
			"http://example.com": "Test Page",
		},
		"/html/body/a/@href": {
			"http://example.com": "/link",
		},
		// The non-matching XPath should result in an empty inner map
		"//nonexistent": {},
		// The custom tag XPath should match
		"//jsLiteral": {
			"http://customtag.com": "some data",
		},
	}

	// --- Execute the function under test ---
	actualOutput, err := processInput(inputJsonBytes)

	// --- Assertions ---
	if err != nil {
		t.Fatalf("processInput returned an unexpected error: %v", err)
	}

	// Use reflect.DeepEqual for comparing maps/structs
	if !reflect.DeepEqual(expectedOutput, actualOutput) {
		// For better debugging, marshal both to JSON strings for comparison output
		expectedJson, _ := json.MarshalIndent(expectedOutput, "", "  ")
		actualJson, _ := json.MarshalIndent(actualOutput, "", "  ")
		t.Errorf("Unexpected output.\nExpected:\n%s\nGot:\n%s", string(expectedJson), string(actualJson))
	}
}

// Test case for invalid input JSON
func TestProcessInput_InvalidJson(t *testing.T) {
	inputJsonBytes := []byte(`{invalid json`)

	_, err := processInput(inputJsonBytes)

	if err == nil {
		t.Fatalf("Expected an error for invalid JSON input, but got nil")
	}
	// Optionally, check if the error message contains expected text
	// e.g., if !strings.Contains(err.Error(), "unexpected end of JSON input") { ... }
}

// Test case for invalid XPath
func TestProcessInput_InvalidXPath(t *testing.T) {
	// Note: Invalid XPaths currently only log a warning to stderr and are skipped.
	// This test verifies the output structure reflects the skipped path.
	inputJsonBytes := []byte(`{
		"xpaths": ["/html/body/p", "[invalid-xpath"],
		"urls": {
			"http://example.com": {
				"content": "<html><body><p>Hello</p></body></html>"
			}
		}
	}`)

	expectedOutput := OutputJson{
		"/html/body/p": {
			"http://example.com": "Hello",
		},
		// The key for the invalid path exists, but the inner map is empty
		// because the path was never added to compiledPaths or processed.
		"[invalid-xpath": {},
	}

	// We don't capture stderr here, but we could if needed.
	actualOutput, err := processInput(inputJsonBytes)

	if err != nil {
		t.Fatalf("processInput returned an unexpected error: %v", err)
	}

	if !reflect.DeepEqual(expectedOutput, actualOutput) {
		expectedJson, _ := json.MarshalIndent(expectedOutput, "", "  ")
		actualJson, _ := json.MarshalIndent(actualOutput, "", "  ")
		t.Errorf("Unexpected output for invalid XPath.\nExpected:\n%s\nGot:\n%s", string(expectedJson), string(actualJson))
	}
}
