use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};

// --- Input Structures ---

#[derive(Deserialize, Debug)]
struct InputJson {
    xpaths: Vec<String>,
    urls: HashMap<String, UrlData>,
}

#[derive(Deserialize, Debug)]
struct UrlData {
    // We don't need targets or content for the stub
    // targets: HashMap<String, String>,
    // content: String,
}

// --- Output Structures ---

#[derive(Serialize, Debug)]
struct OutputJson {
    #[serde(flatten)]
    results: HashMap<String, XpathResult>,
}

#[derive(Serialize, Debug)]
struct XpathResult {
    successful: Vec<String>,
    unsuccessful: Vec<String>,
}

fn process_input(input: InputJson) -> OutputJson {
    // 3. Extract URLs (Moved from main)
    let all_urls: Vec<String> = input.urls.keys().cloned().collect();

    // 4. Build output structure (Moved from main)
    let mut output_results = HashMap::new();
    for xpath in input.xpaths {
        let result = XpathResult {
            // Stub: All URLs are successful for every XPath
            successful: all_urls.clone(),
            unsuccessful: Vec::new(), // Empty unsuccessful list
        };
        output_results.insert(xpath.clone(), result); // Clone xpath here
    }

    OutputJson {
        results: output_results,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read stdin
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    // 2. Deserialize input
    let input: InputJson = serde_json::from_str(&buffer)?;

    // --- Call the processing function ---
    let output = process_input(input);
    // --- End call ---

    // 5. Serialize output
    let output_json_string = serde_json::to_string_pretty(&output)?; // Use pretty print for readability

    // 6. Print to stdout
    println!("{}", output_json_string);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from the parent module (main)

    #[test]
    fn test_process_input_stub() {
        // 1. Prepare input data from JSON string
        let input_json_string = r#"
        {
            "xpaths": [
                "/html/body/h1",
                "//div[@id='main']"
            ],
            "urls": {
                "http://example.com": {},
                "http://anothersite.org": {}
            }
        }
        "#;
        let input: InputJson = serde_json::from_str(input_json_string)
            .expect("Failed to parse test input JSON");

        // 2. Call the function under test
        let output = process_input(input);

        // 3. Define expected output
        let expected_urls = vec!["http://anothersite.org".to_string(), "http://example.com".to_string()];
        // Sort the URLs because HashMap iteration order is not guaranteed
        let mut sorted_expected_urls = expected_urls;
        sorted_expected_urls.sort();

        let mut expected_results = HashMap::new();
        expected_results.insert(
            "/html/body/h1".to_string(),
            XpathResult {
                successful: sorted_expected_urls.clone(), // Use sorted list
                unsuccessful: Vec::new(),
            },
        );
        expected_results.insert(
            "//div[@id='main']".to_string(),
            XpathResult {
                successful: sorted_expected_urls.clone(), // Use sorted list
                unsuccessful: Vec::new(),
            },
        );

        // 4. Assertions
        assert_eq!(output.results.len(), expected_results.len());

        for (xpath, result) in output.results {
            let expected_result = expected_results.get(&xpath).expect("XPath key mismatch");

            // Sort the actual successful URLs before comparison
            let mut sorted_actual_successful = result.successful;
            sorted_actual_successful.sort();

            assert_eq!(sorted_actual_successful, expected_result.successful, "Successful URLs mismatch for XPath: {}", xpath);
            assert_eq!(result.unsuccessful, expected_result.unsuccessful, "Unsuccessful URLs mismatch for XPath: {}", xpath);
        }
    }
}
