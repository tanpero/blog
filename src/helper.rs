use tokio::fs;
use std::path::Path;
use chrono::{DateTime, Local, NaiveDate, Datelike, Timelike};

pub async fn read_file(path: impl AsRef<Path>) -> String {
    match fs::read_to_string(path).await {
        Ok(content) => content,
        Err(e) => format!("Error reading file: {}", e),
    }
}


pub fn format_system_time(time: std::time::SystemTime) -> (String, String) {
    // 将 SystemTime 转换为 chrono 的 DateTime
    let datetime: DateTime<Local> = time.into();

    // 格式化为英文格式
    let english_format = format!(
        "{}, {}, {} {}. {}",
        datetime.format("%A"), // 星期几（完整名称）
        datetime.format("%Y"), // 年
        datetime.format("%B"), // 月份（完整名称）
        ordinal_suffix(datetime.day()), // 日 + 序数后缀
        datetime.format("%I:%M %p") // 时间（12小时制，带上午/下午）
    );

    // 格式化为中文格式
    let chinese_format = format!(
        "{}年{}月{}日，{}，{}",
        chinese_year(datetime.year()),                   // 年
        number_to_chinese(datetime.month()),             // 月
        number_to_chinese(datetime.day()),               // 日
        chinese_weekday(datetime.weekday()),             // 星期几（中文）
        chinese_time(datetime.hour(), datetime.minute()) // 时间（中文格式）
    );

    (english_format, chinese_format)
}

// 为日期添加序数后缀（1st, 2nd, 3rd, 4th 等）
fn ordinal_suffix(day: u32) -> String {
    match day {
        1 | 21 | 31 => format!("{}st", day),
        2 | 22 => format!("{}nd", day),
        3 | 23 => format!("{}rd", day),
        _ => format!("{}th", day),
    }
}

// 将星期几转换为中文
fn chinese_weekday(weekday: chrono::Weekday) -> String {
    match weekday {
        chrono::Weekday::Mon => "星期一".to_string(),
        chrono::Weekday::Tue => "星期二".to_string(),
        chrono::Weekday::Wed => "星期三".to_string(),
        chrono::Weekday::Thu => "星期四".to_string(),
        chrono::Weekday::Fri => "星期五".to_string(),
        chrono::Weekday::Sat => "星期六".to_string(),
        chrono::Weekday::Sun => "星期日".to_string(),
    }
}

// 将年份转换为汉字格式（如：2025 -> 二零二五）
fn chinese_year(year: i32) -> String {
    let s = year.to_string();
    let digits = s.chars().map(|c| match c {
        '0' => "零",
        '1' => "一",
        '2' => "二",
        '3' => "三",
        '4' => "四",
        '5' => "五",
        '6' => "六",
        '7' => "七",
        '8' => "八",
        '9' => "九",
        _ => unreachable!(),
    });

    digits.collect()
}

// 将数字转换为汉字（0-99）
fn number_to_chinese(num: u32) -> String {
    match num {
        0 => "零".to_string(),
        1..=9 => vec!["一", "二", "三", "四", "五", "六", "七", "八", "九"][num as usize - 1].to_string(),
        10 => "十".to_string(),
        11..=19 => format!("十{}", vec!["一", "二", "三", "四", "五", "六", "七", "八", "九"][num as usize - 11]),
        20..=99 => {
            let tens = num / 10;
            let ones = num % 10;
            format!(
                "{}十{}",
                vec!["二", "三", "四", "五", "六", "七", "八", "九"][tens as usize - 2],
                if ones == 0 { "".to_string() } else { vec!["一", "二", "三", "四", "五", "六", "七", "八", "九"][ones as usize - 1].to_string() }
            )
        },
        _ => num.to_string(), // 超出范围直接返回数字字符串
    }
}

// 将小时和分钟转换为汉字
fn chinese_time(hour: u32, minute: u32) -> String {
    let period = match hour {
        23.. => "深夜",
        0..=2 => "凌晨",
        2..=7 => "凌晨",
        7..=12 => "上午",
        12..=15 => "中午",
        15..=18 => "下午",
        18..=20 => "傍晚",
        20..=23 => "晚上",
        _ => unreachable!(), // 不可能到达的分支
    };

    // 将小时和分钟转换为汉字
    let hour = if hour > 12 { hour - 12 } else { hour };
    format!("{}{}点{}分", period, number_to_chinese(hour), number_to_chinese(minute))
}

