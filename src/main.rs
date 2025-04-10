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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read stdin
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    // 2. Deserialize input
    let input: InputJson = serde_json::from_str(&buffer)?;

    // 3. Extract URLs
    let all_urls: Vec<String> = input.urls.keys().cloned().collect();

    // 4. Build output structure
    let mut output_results = HashMap::new();
    for xpath in input.xpaths {
        let result = XpathResult {
            // Stub: All URLs are successful for every XPath
            successful: all_urls.clone(),
            unsuccessful: Vec::new(), // Empty unsuccessful list
        };
        output_results.insert(xpath, result);
    }

    let output = OutputJson {
        results: output_results,
    };

    // 5. Serialize output
    let output_json_string = serde_json::to_string_pretty(&output)?; // Use pretty print for readability

    // 6. Print to stdout
    println!("{}", output_json_string);

    Ok(())
}
