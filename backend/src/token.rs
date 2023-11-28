use tokenizers::tokenizer::{Result, Tokenizer};

const PATH_TO_TOKENIZER: &'static str = "/home/zack/personal/jake/core/mistral/tokenizer.json";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_tokenize() {
        let tokenizer = Tokenizer::from_file(PATH_TO_TOKENIZER).unwrap();

        let encoding = tokenizer.encode("Hey there!", true).unwrap();
        assert_eq!(encoding.get_tokens().len(), 4);
    }
}
