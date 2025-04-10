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
    // We don't need targets for the stub
    // targets: HashMap<String, String>,
    content: String,
}

// --- Output Structures ---


#[derive(Serialize, Debug)]
struct XpathResult {
    successful: Vec<String>,
    unsuccessful: Vec<String>,
}

fn process_input(input: InputJson) -> HashMap<String, XpathResult> {
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

    output_results
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read stdin
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    // 2. Deserialize input
    let input: InputJson = serde_json::from_str(&buffer)?;

    // --- Call the processing function ---
    let output: HashMap<String, XpathResult> = process_input(input);
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
        let output: HashMap<String, XpathResult> = process_input(input);

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
        assert_eq!(output.len(), expected_results.len());

        for (xpath, result) in output {
            let expected_result = expected_results.get(&xpath).expect("XPath key mismatch");

            // Sort the actual successful URLs before comparison
            let mut sorted_actual_successful = result.successful;
            sorted_actual_successful.sort();

            assert_eq!(sorted_actual_successful, expected_result.successful, "Successful URLs mismatch for XPath: {}", xpath);
            assert_eq!(result.unsuccessful, expected_result.unsuccessful, "Unsuccessful URLs mismatch for XPath: {}", xpath);
        }

    #[test]
    fn test_process_input_real_logic_expected_failure() {
        // 1. Prepare input data with HTML content
        let input_json_string = r#"
        {
            "xpaths": [
                "/html/body/p", "//a[@id='link1']", "//div[@class='nonexistent']"
            ],
            "urls": {
                "http://site1.com": {
                    "content": "<html><body><p>Site 1 paragraph</p><a id='link1'>Link 1</a></body></html>"
                },
                "http://site2.com": {
                    "content": "<html><body><p>Site 2 paragraph</p><b>No link here</b></body></html>"
                }
            }
        }
        "#;
        let input: InputJson = serde_json::from_str(input_json_string)
            .expect("Failed to parse test input JSON");

        // 2. Call the function under test
        let output: HashMap<String, XpathResult> = process_input(input);

        // 3. Define expected output (based on real logic, not the stub)
        let mut expected_results = HashMap::new();

        // XPath: "/html/body/p" - Should match both
        let mut urls_p = vec!["http://site1.com".to_string(), "http://site2.com".to_string()];
        urls_p.sort();
        expected_results.insert(
            "/html/body/p".to_string(),
            XpathResult {
                successful: urls_p,
                unsuccessful: Vec::new(),
            },
        );

        // XPath: "//a[@id='link1']" - Should match site1 only
        let mut urls_a_succ = vec!["http://site1.com".to_string()];
        urls_a_succ.sort();
        let mut urls_a_unsucc = vec!["http://site2.com".to_string()];
        urls_a_unsucc.sort();
        expected_results.insert(
            "//a[@id='link1']".to_string(),
            XpathResult {
                successful: urls_a_succ,
                unsuccessful: urls_a_unsucc,
            },
        );

        // XPath: "//div[@class='nonexistent']" - Should match none
        let mut urls_div_unsucc = vec!["http://site1.com".to_string(), "http://site2.com".to_string()];
        urls_div_unsucc.sort();
        expected_results.insert(
            "//div[@class='nonexistent']".to_string(),
            XpathResult {
                successful: Vec::new(),
                unsuccessful: urls_div_unsucc,
            },
        );

        // 4. Assertions (These are expected to fail with the current stub implementation)
        assert_eq!(output.len(), expected_results.len(), "Number of XPaths in output mismatch");

        for (xpath, result) in output {
            let expected_result = expected_results.get(&xpath)
                .expect(&format!("Unexpected XPath key in output: {}", xpath));

            // Sort actual results for comparison
            let mut sorted_actual_successful = result.successful;
            sorted_actual_successful.sort();
            let mut sorted_actual_unsuccessful = result.unsuccessful;
            sorted_actual_unsuccessful.sort();

            // Compare sorted lists
            assert_eq!(sorted_actual_successful, expected_result.successful, "Successful URLs mismatch for XPath: {}", xpath);
            assert_eq!(sorted_actual_unsuccessful, expected_result.unsuccessful, "Unsuccessful URLs mismatch for XPath: {}", xpath);
        }
    }
}
}
