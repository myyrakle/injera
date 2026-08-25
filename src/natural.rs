//! Natural ordering for file names, so `file-2` sorts before `file-10`.

use std::{cmp::Ordering, iter::Peekable, str::Chars};

pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();

    loop {
        match (left.peek(), right.peek()) {
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let ordering = compare_number_chunks(&mut left, &mut right);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(_), Some(_)) => {
                let left_char = left.next().expect("peeked char should exist");
                let right_char = right.next().expect("peeked char should exist");
                let ordering = left_char.cmp(&right_char);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => return Ordering::Equal,
        }
    }
}

fn compare_number_chunks(
    left: &mut Peekable<Chars<'_>>,
    right: &mut Peekable<Chars<'_>>,
) -> Ordering {
    let left_number = take_ascii_digits(left);
    let right_number = take_ascii_digits(right);
    let left_trimmed = left_number.trim_start_matches('0');
    let right_trimmed = right_number.trim_start_matches('0');
    let left_normalized = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_normalized = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };

    left_normalized
        .len()
        .cmp(&right_normalized.len())
        .then_with(|| left_normalized.cmp(right_normalized))
        .then_with(|| left_number.len().cmp(&right_number.len()))
}

fn take_ascii_digits(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut digits = String::new();

    while chars.peek().is_some_and(|char| char.is_ascii_digit()) {
        digits.push(chars.next().expect("peeked char should exist"));
    }

    digits
}
