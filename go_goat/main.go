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

// --- Main Logic ---

func main() {
	// 1. Read stdin
	inputBytes, err := io.ReadAll(os.Stdin)
	if err != nil {
		fatalf("Error reading stdin: %v\n", err)
	}

	// 2. Deserialize input
	var input InputJson
	err = json.Unmarshal(inputBytes, &input)
	if err != nil {
		fatalf("Error unmarshalling input JSON: %v\n", err)
	}

	// 3. Process XPaths and URLs
	output := make(OutputJson)

	for _, xpathStr := range input.Xpaths {
		// Compile XPath expression once per XPath
		path, err := xmlpath.Compile(xpathStr)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Warning: Failed to compile XPath '%s': %v. Skipping.\n", xpathStr, err)
			continue // Skip this XPath if it's invalid
		}

		// Initialize the inner map for this XPath
		output[xpathStr] = make(map[string]string)

		for url, urlData := range input.Urls {
			// Create a reader for the HTML/XML content string
			contentReader := strings.NewReader(urlData.Content)

			// Decode the content
			root, err := decode(contentReader)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Warning: Failed to parse content for URL '%s': %v. Skipping URL for XPath '%s'.\n", url, err, xpathStr)
				output[xpathStr][url] = fmt.Sprintf("Error parsing content: %v", err) // Record error
				continue
			}

			// Evaluate the XPath
			resultBytes, ok := path.Bytes(root)
			if !ok {
				// No match found is not necessarily an error, represent as empty string or specific marker?
				// For now, let's use an empty string to indicate no match.
				output[xpathStr][url] = ""
				// fmt.Fprintf(os.Stderr, "[go_goat: no items selected for XPath '%s' in URL '%s']\n", xpathStr, url)
			} else {
				output[xpathStr][url] = string(resultBytes)
			}
		}
	}

	// 4. Serialize output
	outputJsonBytes, err := json.MarshalIndent(output, "", "  ") // Use indent for readability
	if err != nil {
		fatalf("Error marshalling output JSON: %v\n", err)
	}

	// 5. Print to stdout
	fmt.Println(string(outputJsonBytes))
}
