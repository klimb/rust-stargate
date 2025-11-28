// Pipeline parsing
pub fn parse_pipeline(input: &str) -> Vec<Vec<String>> {
    let mut pipelines = Vec::new();
    let mut current_cmd = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in input.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            '"' | '\'' if in_quotes && ch == quote_char => {
                in_quotes = false;
            }
            '|' if !in_quotes => {
                if !current_arg.is_empty() {
                    current_cmd.push(current_arg.clone());
                    current_arg.clear();
                }
                if !current_cmd.is_empty() {
                    pipelines.push(current_cmd.clone());
                    current_cmd.clear();
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current_arg.is_empty() {
                    current_cmd.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => {
                current_arg.push(ch);
            }
        }
    }

    if !current_arg.is_empty() {
        current_cmd.push(current_arg);
    }
    if !current_cmd.is_empty() {
        pipelines.push(current_cmd);
    }

    pipelines
}
