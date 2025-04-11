package main

import (
	"encoding/json"
	"encoding/xml"
	"fmt"
	"io"
	"os"
	"strings"

	"golang.org/x/net/html/charset" // For character encoding detection
	"launchpad.net/xmlpath"        // The XPath library used by xpup
)

// --- Input Structures ---

type InputJson struct {
	Xpaths []string          `json:"xpaths"`
	Urls   map[string]UrlData `json:"urls"`
}

type UrlData struct {
	Content string `json:"content"`
}

// --- Output Structures ---

// Output format: map[xpath]map[url]result
type OutputJson map[string]map[string]string

// --- Helper Functions ---

func fatalf(format string, a ...interface{}) {
	fmt.Fprintf(os.Stderr, format, a...)
	os.Exit(2)
}

// decode reads from the reader, attempts to detect charset, and parses XML
func decode(r io.Reader) (*xmlpath.Node, error) {
	decoder := xml.NewDecoder(r)
	// Use charset reader similar to xpup to handle different encodings
	decoder.CharsetReader = func(chset string, input io.Reader) (io.Reader, error) {
		// xmlpath doesn't seem to expose the underlying reader easily after parsing starts,
		// so we rely on the standard library's decoder CharsetReader.
		// If charset is empty, it might try to auto-detect or default to UTF-8.
		return charset.NewReader(input, chset)
	}
	return xmlpath.ParseDecoder(decoder)
}

// --- Processing Logic ---

// processInput takes raw input bytes, processes them, and returns the result map or an error.
func processInput(inputBytes []byte) (OutputJson, error) {
	// 1. Deserialize input
	var input InputJson
	err := json.Unmarshal(inputBytes, &input)
	if err != nil {
		// Return an error instead of exiting
		return nil, fmt.Errorf("error unmarshalling input JSON: %w", err)
	}

	// 2. Initialize Output and Compile XPaths
	output := make(OutputJson)
	compiledPaths := make(map[string]*xmlpath.Path) // Store compiled XPaths

	for _, xpathStr := range input.Xpaths {
		// Initialize the inner map for this XPath in the output
		output[xpathStr] = make(map[string]string)

		// Compile XPath expression
		path, err := xmlpath.Compile(xpathStr)
		if err != nil {
			// Log warning, but don't stop processing other paths/URLs
			fmt.Fprintf(os.Stderr, "Warning: Failed to compile XPath '%s': %v. Skipping this XPath for all URLs.\n", xpathStr, err)
			// We skip adding it to compiledPaths, so it won't be processed.
		} else {
			compiledPaths[xpathStr] = path
		}
	}

	// 3. Process URLs and Apply Compiled XPaths
	for url, urlData := range input.Urls {
		// Create a reader for the HTML/XML content string
		contentReader := strings.NewReader(urlData.Content)

		// Decode the content *once* per URL
		root, err := decode(contentReader)
		if err != nil {
			// Log warning and skip this URL entirely if parsing fails
			fmt.Fprintf(os.Stderr, "Warning: Failed to parse content for URL '%s': %v. Skipping this URL.\n", url, err)
			continue // Skip to the next URL
		}

		// If root is nil even after successful decode (e.g., empty valid XML), skip URL.
		// xmlpath.ParseDecoder usually returns EOF for empty input, caught above.
		// This check handles edge cases where parsing succeeds but yields no root.
		if root == nil {
			fmt.Fprintf(os.Stderr, "Warning: Parsed content for URL '%s' resulted in nil root node. Skipping this URL.\n", url)
			continue // Skip to the next URL
		}

		// Apply each valid, compiled XPath to this URL's content
		for xpathStr, path := range compiledPaths {
			// Evaluate the XPath on the parsed root
			resultBytes, ok := path.Bytes(root)
			// Only add the entry if the XPath matched and returned bytes
			if ok {
				output[xpathStr][url] = string(resultBytes)
			}
			// If 'ok' is false (no match or non-byte result), do nothing - omit the entry.
		}
	}

	return output, nil // Return the populated map and nil error if successful so far
}

// --- Main Function ---

func main() {
	// 1. Read stdin
	inputBytes, err := io.ReadAll(os.Stdin)
	if err != nil {
		fatalf("Error reading stdin: %v\n", err) // Use fatalf for I/O errors in main
	}

	// 2. Process Input using the dedicated function
	output, err := processInput(inputBytes)
	if err != nil {
		// Handle fatal errors from processing (e.g., JSON parsing)
		fatalf("Error processing input: %v\n", err)
	}

	// 3. Serialize output
	outputJsonBytes, err := json.MarshalIndent(output, "", "  ") // Use indent for readability
	if err != nil {
		fatalf("Error marshalling output JSON: %v\n", err) // Use fatalf for marshalling errors
	}

	// 4. Print to stdout
	fmt.Println(string(outputJsonBytes))
}
