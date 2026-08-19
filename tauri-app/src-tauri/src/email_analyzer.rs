use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct EmailAnalysis {
    pub verification_code: Option<String>,
    pub important_keywords: Vec<String>,
    pub category: EmailCategory,
}

#[derive(Debug, Clone)]
pub enum EmailCategory {
    Verification,
    Important,
    Normal,
}

pub struct EmailAnalyzer {
    verification_patterns: Vec<Regex>,
    important_keywords: Vec<String>,
}

impl EmailAnalyzer {
    pub fn new() -> Self {
        let verification_patterns = vec![
            // 字母数字混合验证码（优先级最高）
            // 验证码：a03hg2
            Regex::new(r"验证码[：:]\s*([a-zA-Z0-9]{4,8})").unwrap(),
            // 验证码为：a03hg2
            Regex::new(r"验证码为[：:]\s*([a-zA-Z0-9]{4,8})").unwrap(),
            // 验证码是：a03hg2
            Regex::new(r"验证码是[：:]\s*([a-zA-Z0-9]{4,8})").unwrap(),
            // Your verification code is a03hg2
            Regex::new(r"(?i)verification code is\s*([a-zA-Z0-9]{4,8})").unwrap(),
            // Code: a03hg2
            Regex::new(r"(?i)code[：:]\s*([a-zA-Z0-9]{4,8})").unwrap(),
            // 您的验证码 a03hg2
            Regex::new(r"您的验证码\s*([a-zA-Z0-9]{4,8})").unwrap(),
            // a03hg2 是您的验证码
            Regex::new(r"([a-zA-Z0-9]{4,8})\s*是您的验证码").unwrap(),
            // a03hg2 为您的验证码
            Regex::new(r"([a-zA-Z0-9]{4,8})\s*为您的验证码").unwrap(),
            // 验证码 a03hg2
            Regex::new(r"验证码\s+([a-zA-Z0-9]{4,8})").unwrap(),
            // 【a03hg2】（验证码上下文）
            Regex::new(r"验证码.*?【([a-zA-Z0-9]{4,8})】").unwrap(),
            // 验证码(a03hg2)
            Regex::new(r"验证码.*?\(([a-zA-Z0-9]{4,8})\)").unwrap(),
            // 验证码为 a03hg2
            Regex::new(r"验证码为\s+([a-zA-Z0-9]{4,8})").unwrap(),
            // 验证码是 a03hg2
            Regex::new(r"验证码是\s+([a-zA-Z0-9]{4,8})").unwrap(),

            // 纯数字验证码（保持兼容）
            // 验证码：123456
            Regex::new(r"验证码[：:]\s*(\d{4,8})").unwrap(),
            // 验证码 123456
            Regex::new(r"验证码\s+(\d{4,8})").unwrap(),
            // 验证码是123456
            Regex::new(r"验证码是\s*(\d{4,8})").unwrap(),
            // Your verification code is 123456
            Regex::new(r"(?i)verification code is\s*(\d{4,8})").unwrap(),
            // Code: 123456
            Regex::new(r"(?i)code[：:]\s*(\d{4,8})").unwrap(),
            // 动态密码：123456
            Regex::new(r"动态密码[：:]\s*(\d{4,8})").unwrap(),
            // 短信验证码：123456
            Regex::new(r"短信验证码[：:]\s*(\d{4,8})").unwrap(),
            // 验证码为：123456
            Regex::new(r"验证码为[：:]\s*(\d{4,8})").unwrap(),
            // 【123456】（验证码上下文）
            Regex::new(r"验证码.*?【(\d{4,8})】").unwrap(),
            // 验证码(123456)
            Regex::new(r"验证码.*?\((\d{4,8})\)").unwrap(),
            // 验证码 123456 （带空格）
            Regex::new(r"验证码\s*(\d{4,8})\s").unwrap(),
            // 123456 是您的验证码
            Regex::new(r"(\d{4,8})\s*是您的验证码").unwrap(),
            // 123456 为您的验证码
            Regex::new(r"(\d{4,8})\s*为您的验证码").unwrap(),
            // 您的验证码 123456
            Regex::new(r"您的验证码\s*(\d{4,8})").unwrap(),
            // 本次验证码：123456
            Regex::new(r"本次验证码[：:]\s*(\d{4,8})").unwrap(),
            // 登录验证码：123456
            Regex::new(r"登录验证码[：:]\s*(\d{4,8})").unwrap(),
            // 手机验证码：123456
            Regex::new(r"手机验证码[：:]\s*(\d{4,8})").unwrap(),
            // 邮箱验证码：123456
            Regex::new(r"邮箱验证码[：:]\s*(\d{4,8})").unwrap(),
            // 安全验证码：123456
            Regex::new(r"安全验证码[：:]\s*(\d{4,8})").unwrap(),
            // OTP: 123456
            Regex::new(r"(?i)otp[：:]\s*(\d{4,8})").unwrap(),
            // PIN: 123456
            Regex::new(r"(?i)pin[：:]\s*(\d{4,8})").unwrap(),
            // 验证码123456
            Regex::new(r"验证码(\d{4,8})").unwrap(),
            // 数字验证码：123456
            Regex::new(r"数字验证码[：:]\s*(\d{4,8})").unwrap(),
            // 临时密码：123456
            Regex::new(r"临时密码[：:]\s*(\d{4,8})").unwrap(),
            // 一次性密码：123456
            Regex::new(r"一次性密码[：:]\s*(\d{4,8})").unwrap(),
            // 动态口令：123456
            Regex::new(r"动态口令[：:]\s*(\d{4,8})").unwrap(),
            // 6位数字：123456
            Regex::new(r"6位数字[：:]\s*(\d{6})").unwrap(),
            // 4位数字：1234
            Regex::new(r"4位数字[：:]\s*(\d{4})").unwrap(),
            // 验证码为 123456
            Regex::new(r"验证码为\s+(\d{4,8})").unwrap(),
            // 验证码是 123456
            Regex::new(r"验证码是\s+(\d{4,8})").unwrap(),
        ];

        let important_keywords = vec![
            // 金融相关
            "银行".to_string(),
            "支付".to_string(),
            "转账".to_string(),
            "余额".to_string(),
            "账单".to_string(),
            "信用卡".to_string(),
            "还款".to_string(),
            "逾期".to_string(),
            
            // 安全相关
            "密码".to_string(),
            "登录".to_string(),
            "安全".to_string(),
            "异常".to_string(),
            "风险".to_string(),
            "冻结".to_string(),
            "解冻".to_string(),
            
            // 工作相关
            "会议".to_string(),
            "紧急".to_string(),
            "重要".to_string(),
            "截止".to_string(),
            "deadline".to_string(),
            "urgent".to_string(),
            "important".to_string(),
            
            // 购物相关
            "订单".to_string(),
            "发货".to_string(),
            "快递".to_string(),
            "退款".to_string(),
            "售后".to_string(),
            
            // 服务相关
            "到期".to_string(),
            "续费".to_string(),
            "升级".to_string(),
            "维护".to_string(),
            "通知".to_string(),
        ];

        Self {
            verification_patterns,
            important_keywords,
        }
    }

    pub fn analyze_email(&self, subject: &str, body: &str) -> EmailAnalysis {
        let full_text = format!("{} {}", subject, body);
        
        // 提取验证码
        let verification_code = self.extract_verification_code(&full_text);
        
        // 提取重要关键词
        let important_keywords = self.extract_important_keywords(&full_text);
        
        // 确定邮件类别
        let category = if verification_code.is_some() {
            EmailCategory::Verification
        } else if !important_keywords.is_empty() {
            EmailCategory::Important
        } else {
            EmailCategory::Normal
        };

        EmailAnalysis {
            verification_code,
            important_keywords,
            category,
        }
    }

    fn extract_verification_code(&self, text: &str) -> Option<String> {
        for pattern in &self.verification_patterns {
            if let Some(captures) = pattern.captures(text) {
                if let Some(code) = captures.get(1) {
                    let code_str = code.as_str();
                    // 验证码长度通常在4-8位之间
                    if code_str.len() >= 4 && code_str.len() <= 8 {
                        // 验证是否为有效的验证码格式
                        if self.is_valid_verification_code(code_str) {
                            return Some(code_str.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn is_valid_verification_code(&self, code: &str) -> bool {
        // 排除常见的非验证码词汇
        let excluded_words = [
            "twitter", "facebook", "google", "apple", "microsoft", "amazon",
            "github", "linkedin", "instagram", "youtube", "tiktok", "wechat",
            "alipay", "taobao", "baidu", "tencent", "alibaba", "xiaomi",
            "huawei", "oppo", "vivo", "meizu", "lenovo", "asus", "dell",
            "admin", "user", "test", "demo", "sample", "example", "hello",
            "world", "china", "beijing", "shanghai", "guangzhou", "shenzhen"
        ];

        let code_lower = code.to_lowercase();
        if excluded_words.contains(&code_lower.as_str()) {
            return false;
        }

        // 纯数字验证码：4-8位数字
        if code.chars().all(|c| c.is_ascii_digit()) {
            return code.len() >= 4 && code.len() <= 8;
        }

        // 字母数字混合验证码：4-8位，必须包含至少一个数字
        if code.chars().all(|c| c.is_ascii_alphanumeric()) {
            let has_digit = code.chars().any(|c| c.is_ascii_digit());
            let has_letter = code.chars().any(|c| c.is_ascii_alphabetic());

            // 验证码必须包含数字，或者是纯字母但长度较短（4-6位）
            if has_digit {
                return code.len() >= 4 && code.len() <= 8;
            } else if has_letter && code.len() >= 4 && code.len() <= 6 {
                // 纯字母的情况，长度限制更严格，且不能是常见单词
                return !self.is_common_word(&code_lower);
            }
        }

        false
    }

    fn is_common_word(&self, word: &str) -> bool {
        // 常见英文单词列表（验证码不太可能是这些词）
        let common_words = [
            "code", "pass", "word", "mail", "send", "from", "dear", "hello",
            "thank", "please", "click", "here", "link", "visit", "check",
            "verify", "confirm", "account", "login", "register", "welcome"
        ];

        common_words.contains(&word)
    }

    fn extract_important_keywords(&self, text: &str) -> Vec<String> {
        let mut found_keywords = Vec::new();
        let text_lower = text.to_lowercase();
        
        for keyword in &self.important_keywords {
            if text_lower.contains(&keyword.to_lowercase()) {
                found_keywords.push(keyword.clone());
            }
        }
        
        // 去重并限制数量
        found_keywords.sort();
        found_keywords.dedup();
        found_keywords.truncate(3); // 最多显示3个关键词
        
        found_keywords
    }

    pub fn get_display_tags(&self, analysis: &EmailAnalysis) -> Vec<EmailTag> {
        let mut tags = Vec::new();

        // 验证码标签
        if let Some(code) = &analysis.verification_code {
            tags.push(EmailTag {
                text: format!("验证码: {}", code),
                tag_type: TagType::VerificationCode,
                color: "#e74c3c".to_string(),
            });
        }

        // 重要关键词标签
        if !analysis.important_keywords.is_empty() {
            let keywords_text = if analysis.important_keywords.len() == 1 {
                analysis.important_keywords[0].clone()
            } else {
                format!("{}等", analysis.important_keywords[0])
            };
            
            tags.push(EmailTag {
                text: keywords_text,
                tag_type: TagType::Important,
                color: "#f39c12".to_string(),
            });
        }

        // 如果没有特殊标签，显示文件夹标签
        if tags.is_empty() {
            tags.push(EmailTag {
                text: "收件箱".to_string(),
                tag_type: TagType::Folder,
                color: "#95a5a6".to_string(),
            });
        }

        tags
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTag {
    pub text: String,
    pub tag_type: TagType,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagType {
    VerificationCode,
    Important,
    Folder,
}

impl Default for EmailAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_code_extraction() {
        let analyzer = EmailAnalyzer::new();

        let test_cases = vec![
            // 纯数字验证码
            ("您的验证码：123456", Some("123456")),
            ("验证码是 654321", Some("654321")),
            ("Your verification code is 789012", Some("789012")),
            ("【567890】是您的验证码", Some("567890")),

            // 字母数字混合验证码
            ("您的验证码为: a03hg2", Some("a03hg2")),
            ("验证码：abc123", Some("abc123")),
            ("验证码是：x9y8z7", Some("x9y8z7")),
            ("Your verification code is A1B2C3", Some("A1B2C3")),
            ("Code: m5n6p7", Some("m5n6p7")),
            ("【k4j5h6】", Some("k4j5h6")),

            // 无效情况
            ("没有验证码的邮件", None),
            ("验证码太短：ab", None),
            ("验证码太长：abcdefghijk", None),
            ("【Twitter】", None), // 常见词汇不应被识别为验证码
            ("(GitHub)", None),   // 常见词汇不应被识别为验证码
            ("【Google】", None),  // 常见词汇不应被识别为验证码
            ("验证码【Twitter】", None), // 即使有验证码上下文，常见词汇也不应被识别
        ];

        for (text, expected) in test_cases {
            let result = analyzer.extract_verification_code(text);
            assert_eq!(result.as_deref(), expected, "Failed for text: {}", text);
        }
    }

    #[test]
    fn test_important_keywords_extraction() {
        let analyzer = EmailAnalyzer::new();
        
        let text = "您的银行账户余额不足，请及时还款";
        let keywords = analyzer.extract_important_keywords(text);
        
        assert!(keywords.contains(&"银行".to_string()));
        assert!(keywords.contains(&"余额".to_string()));
        assert!(keywords.contains(&"还款".to_string()));
    }
}
