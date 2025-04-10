use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};
use sxd_document::parser;
use sxd_xpath::{evaluate_xpath, Factory, Context, Value};

// --- Input Structures ---

#[derive(Deserialize, Debug)]
struct InputJson {
    xpaths: HashMap<String, Vec<String>>,
    urls: HashMap<String, UrlData>,
}

#[derive(Deserialize, Debug)]
struct UrlData {
    // We don't need targets for the stub
    targets: HashMap<String, String>,
    content: String,
}

// --- Output Structures ---

#[derive(Serialize, Debug)]
struct XpathResult {
    successful: Vec<String>,
    unsuccessful: Vec<String>,
}

fn process_input(input: InputJson) -> HashMap<String, XpathResult> {
    // Pre-parse all HTML documents
    let packages: HashMap<String, Result<sxd_document::Package, _>> = input
        .urls
        .iter()
        .map(|(url, url_data)| (url.clone(), parser::parse(&url_data.content)))
        .collect();

    let mut output_results = HashMap::new();
    let xpath_factory = Factory::new();

    // Iterate through headings and their associated XPath lists
    for (heading, xpath_list) in &input.xpaths {
        // Iterate through individual XPath strings in the list
        for xpath_str in xpath_list {
            let mut successful_urls = Vec::new();
            let mut unsuccessful_urls = Vec::new();

            // Attempt to compile the XPath expression once
            let xpath = match xpath_factory.build(xpath_str) {
                Ok(xp) => Some(xp),
                Err(_) => {
                    // If XPath compilation fails, all URLs are unsuccessful for this XPath
                    None
                }
            };

            // Iterate through each URL to check this XPath
            for (url_string, url_data) in &input.urls {
                let expected_target = url_data.targets.get(heading).map(|s| s.as_str()).unwrap_or("");

                match (packages.get(url_string).unwrap(), &xpath) {
                    (Ok(package), Some(compiled_xpath)) => {
                        let document = package.as_document();
                        let context = Context::new();
                        match compiled_xpath.evaluate(&context, document.root()) {
                            Ok(Value::String(actual_value)) => {
                                if actual_value == expected_target {
                                    successful_urls.push(url_string.clone());
                                } else {
                                    unsuccessful_urls.push(url_string.clone());
                                }
                            }
                            Ok(_) | Err(_) => {
                                if expected_target.is_empty() && matches!(compiled_xpath.evaluate(&context, document.root()), Ok(Value::Nodeset(nodeset)) if nodeset.size() == 0) {
                                     successful_urls.push(url_string.clone());
                                } else {
                                     unsuccessful_urls.push(url_string.clone());
                                }
                            }
                        }
                    }
                    (Err(_), _) | (_, None) => {
                        unsuccessful_urls.push(url_string.clone());
                    }
                }
            }

            output_results.entry(xpath_str.clone()).or_insert_with(|| XpathResult {
                successful: successful_urls,
                unsuccessful: unsuccessful_urls,
            });
        }
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
            "xpaths": {
                "Heading 1": ["/html/body/h1"],
                "Heading 2": ["//div[@id='main']"]
            },
            "urls": {
                "http://example.com": {
                    "targets": {},
                    "content": ""
                },
                "http://anothersite.org": {
                    "targets": {},
                    "content": ""
                }
            }
        }
        "#;
        let input: InputJson =
            serde_json::from_str(input_json_string).expect("Failed to parse test input JSON");

        // 2. Call the function under test
        let output: HashMap<String, XpathResult> = process_input(input);

        // 3. Define expected output
        let expected_urls = vec![
            "http://anothersite.org".to_string(),
            "http://example.com".to_string(),
        ];
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

            assert_eq!(
                sorted_actual_successful, expected_result.successful,
                "Successful URLs mismatch for XPath: {}",
                xpath
            );
            assert_eq!(
                result.unsuccessful, expected_result.unsuccessful,
                "Unsuccessful URLs mismatch for XPath: {}",
                xpath
            );
        }
    }
    #[test]
    fn test_process_input_real_logic_expected_failure() {
        // 1. Prepare input data with HTML content
        let input_json_string = r#"
        {
            "xpaths": {
                "Content Selectors": ["/html/body/p"],
                "Link Selectors": ["//a[@id='link1']"],
                "Nonexistent Selectors": ["//div[@class='nonexistent']"]
            },
            "urls": {
                "http://site1.com": {
                    "targets": {
                        "Content Selectors": "Site 1 paragraph",
                        "Link Selectors": "Link 1",
                        "Nonexistent Selectors": ""
                    },
                    "content": "<html><body><p>Site 1 paragraph</p><a id='link1'>Link 1</a></body></html>"
                },
                "http://site2.com": {
                    "targets": {
                        "Content Selectors": "Site 2 paragraph",
                        "Link Selectors": "",
                        "Nonexistent Selectors": ""
                    },
                    "content": "<html><body><p>Site 2 paragraph</p><b>No link here</b></body></html>"
                }
            }
        }
        "#;
        let input: InputJson =
            serde_json::from_str(input_json_string).expect("Failed to parse test input JSON");

        // 2. Call the function under test
        let output: HashMap<String, XpathResult> = process_input(input);

        // 3. Define expected output (based on real logic, not the stub)
        let mut expected_results = HashMap::new();

        // XPath: "/html/body/p" - Should match both
        let mut urls_p = vec![
            "http://site1.com".to_string(),
            "http://site2.com".to_string(),
        ];
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
        let mut urls_div_unsucc = vec![
            "http://site1.com".to_string(),
            "http://site2.com".to_string(),
        ];
        urls_div_unsucc.sort();
        expected_results.insert(
            "//div[@class='nonexistent']".to_string(),
            XpathResult {
                successful: Vec::new(),
                unsuccessful: urls_div_unsucc,
            },
        );

        // 4. Assertions (These are expected to fail with the current stub implementation)
        assert_eq!(
            output.len(),
            expected_results.len(),
            "Number of XPaths in output mismatch"
        );

        for (xpath, result) in output {
            let expected_result = expected_results
                .get(&xpath)
                .expect(&format!("Unexpected XPath key in output: {}", xpath));

            // Sort actual results for comparison
            let mut sorted_actual_successful = result.successful;
            sorted_actual_successful.sort();
            let mut sorted_actual_unsuccessful = result.unsuccessful;
            sorted_actual_unsuccessful.sort();

            // Compare sorted lists
            assert_eq!(
                sorted_actual_successful, expected_result.successful,
                "Successful URLs mismatch for XPath: {}",
                xpath
            );
            assert_eq!(
                sorted_actual_unsuccessful, expected_result.unsuccessful,
                "Unsuccessful URLs mismatch for XPath: {}",
                xpath
            );
        }
    }
}
