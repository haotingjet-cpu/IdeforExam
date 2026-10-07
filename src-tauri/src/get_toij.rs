use scraper::{Html, Selector};

#[derive(Debug, Clone)]
struct ATiojProblemOption {
    description: Option<String>,
    input_format: Option<String>,
    output_format: Option<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct ATiojProblem {
    description: String,
    #[serde(rename = "inputFormat")]
    input_format: String,
    #[serde(rename = "outputFormat")]
    output_format: String,
}

impl ATiojProblemOption {
    const NONE: ATiojProblemOption = ATiojProblemOption {
        description: None,
        input_format: None,
        output_format: None,
    };
}

impl From<ATiojProblemOption> for ATiojProblem {
    fn from(v: ATiojProblemOption) -> ATiojProblem {
        ATiojProblem {
            description: v.description.unwrap(),
            input_format: v.input_format.unwrap(),
            output_format: v.output_format.unwrap(),
        }
    }
}

impl ATiojProblem {
    pub fn get(target_url: &str) -> Result<Self, String> {
        println!("連線 tioj 題目中...");

        use std::process::Command;
        let Ok(output) = Command::new("curl").arg("-s").arg(target_url).output() else {
            return Err("curl 命令建立失敗".into());
        };

        if !output.status.success() {
            println!("curl 執行失敗");
            return Err("curl 執行失敗".into());
        }

        let response_text = String::from_utf8_lossy(&output.stdout);
        parse_problem_html(&response_text).map_err(|error| format!("無法解析題目：{error}"))
    }
}

fn parse_problem_html(html: &str) -> Result<ATiojProblem, String> {
    let document = Html::parse_document(html);
    let panel_selector = Selector::parse("div.panel-default").map_err(|error| error.to_string())?;
    let title_selector = Selector::parse("h1.panel-title").map_err(|error| error.to_string())?;
    let body_selector = Selector::parse("div.panel-body").map_err(|error| error.to_string())?;

    let mut problem = ATiojProblemOption::NONE;
    let mut found = [false; 3];

    for panel in document.select(&panel_selector) {
        let Some(title_element) = panel.select(&title_selector).next() else {
            continue;
        };
        let title = title_element.text().collect::<String>().trim().to_string();
        let Some(body_element) = panel.select(&body_selector).next() else {
            continue;
        };

        let value = normalize_problem_text(body_element);
        match title.as_str() {
            "Description" => {
                found[0] = true;
                problem.description = Some(value);
            }
            "Input Format" => {
                found[1] = true;
                problem.input_format = Some(value);
            }
            "Output Format" => {
                found[2] = true;
                problem.output_format = Some(value);
            }
            _ => {}
        }
    }

    if !found.iter().all(|item| *item) {
        return Err("網頁中找不到完整的 Description、Input Format 或 Output Format".into());
    }

    Ok(problem.into())
}

fn normalize_problem_text(element: scraper::ElementRef<'_>) -> String {
    let mut lines: Vec<String> = Vec::new();
    for child in element.children() {
        let Some(child_element) = scraper::ElementRef::wrap(child) else {
            continue;
        };
        let text = child_element.text().collect::<Vec<_>>().concat();
        let cleaned = text
            .replace("\r\n", "\n")
            .replace("\r", "\n")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !cleaned.is_empty() {
            lines.push(cleaned);
        }
    }

    lines.join("\n\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_problem_html;

    #[test]
    fn parses_complete_problem_text_as_plain_text() {
        let html = r#"
            <div class="panel-default">
                <h1 class="panel-title">Description</h1>
                <div class="panel-body">
                    <p>給定 <strong>a</strong> 與 <strong>b</strong>。</p>
                    <p>輸出 $a+b$。</p>
                </div>
            </div>
            <div class="panel-default">
                <h1 class="panel-title">Input Format</h1>
                <div class="panel-body">
                    <p>第一行為 n。</p>
                    <p>第二行為輸入。</p>
                </div>
            </div>
            <div class="panel-default">
                <h1 class="panel-title">Output Format</h1>
                <div class="panel-body">
                    <p>輸出答案。</p>
                </div>
            </div>
        "#;

        let problem = parse_problem_html(html).unwrap();
        assert!(problem.description.starts_with("給定 a 與 b。"));
        assert!(problem.description.contains("輸出 $a+b$。"));
        assert_eq!(
            problem.input_format, "第一行為 n。\n\n第二行為輸入。",
            "input should preserve paragraphs"
        );
        assert_eq!(
            problem.output_format, "輸出答案。",
            "output should be captured"
        );
    }

    #[test]
    fn rejects_incomplete_problem_data() {
        let html = r#"
            <div class="panel-default">
                <h1 class="panel-title">Description</h1>
                <div class="panel-body"><p>題目</p></div>
            </div>
        "#;

        assert!(parse_problem_html(html).is_err());
    }
}
