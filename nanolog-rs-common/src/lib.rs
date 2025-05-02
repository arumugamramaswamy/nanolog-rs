pub mod nanolog_logger;

use core::arch::x86_64::_rdtsc;
use libc::{clock_gettime, timespec, CLOCK_MONOTONIC};
use regex::Regex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use syn::{parse::Parse, token::Comma, Expr};

// #[derive(Debug)]
pub struct Nanolog {
    pub fmt_literal: String,
    pub fmt_specifiers: Vec<NanologType>,
    pub punctuate: syn::punctuated::Punctuated<Expr, Comma>,
    pub sink: Expr,
}

impl Nanolog {
    pub fn get_log_type_suffix(&self) -> String {
        self.fmt_specifiers
            .iter()
            .map(|s| s.to_type_string())
            .collect::<String>()
    }
}

impl Parse for Nanolog {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let sink: syn::Expr = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let fmt_literal: syn::LitStr = input.parse()?;
        let fmt_string = fmt_literal.value();
        let fmt_specifiers = find_format_specifiers(&fmt_string);
        if input.is_empty() {
            if fmt_specifiers.len() != 0 {
                Err(syn::Error::new(
                    fmt_literal.span(),
                    format!("No arguments found. But format specifiers found {fmt_specifiers:?}",),
                ))
            } else {
                Ok(Nanolog {
                    fmt_literal: fmt_string,
                    fmt_specifiers,
                    punctuate: syn::punctuated::Punctuated::<Expr, Comma>::new(),
                    sink,
                })
            }
        } else {
            input.parse::<syn::Token![,]>()?;
            let punctuated =
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated(input)?;
            if punctuated.len() == fmt_specifiers.len() {
                Ok(Nanolog {
                    fmt_literal: fmt_string,
                    fmt_specifiers,
                    punctuate: punctuated,
                    sink,
                })
            } else {
                Err(syn::Error::new(
                    fmt_literal.span(),
                    format!(
                        "Number of format specifiers {fmt_specifiers:?} != Number of arguments {}",
                        punctuated.len()
                    ),
                ))
            }
        }
    }
}

#[derive(Debug)]
pub enum NanologType {
    Int,
    Float,
}

impl NanologType {
    fn to_type_string(&self) -> &str {
        match self {
            NanologType::Int => "D",
            NanologType::Float => "F",
        }
    }
}

fn find_format_specifiers(input: &str) -> Vec<NanologType> {
    let re = Regex::new(r"%[sdf]").unwrap();
    re.find_iter(input)
        .map(|mat| match mat.as_str() {
            "%d" => NanologType::Int,
            "%f" => NanologType::Float,
            _ => unreachable!(),
        })
        .collect()
}

pub const fn const_fnv1a_hash(filename: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037; // FNV offset basis
    let bytes = filename.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(1099511628211); // FNV prime
        i += 1;
    }

    hash
}

pub fn system_time_to_micros(time: SystemTime) -> u64 {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .expect("Time before UNIX_EPOCH");
    duration.as_secs() * 1_000_000 + u64::from(duration.subsec_micros())
}

pub fn get_monotonic_time_micros() -> u64 {
    let mut ts = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    // Call clock_gettime with CLOCK_MONOTONIC
    if unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) } != 0 {
        panic!("Failed to get monotonic time");
    }

    // Convert to microseconds
    (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1_000
}

pub fn get_rdtsc_time() -> u64 {
    unsafe { _rdtsc() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_empty() {
        syn::parse_str::<syn::Macro>("nanolog!(sink, \"??\")")
            .unwrap()
            .parse_body::<Nanolog>()
            .unwrap();
    }

    #[test]
    fn parse_single() {
        syn::parse_str::<syn::Macro>("nanolog!(sink, \"??%d\", a)")
            .unwrap()
            .parse_body::<Nanolog>()
            .unwrap();
    }

    #[test]
    fn parse_fail_no_args() {
        assert!(syn::parse_str::<syn::Macro>("nanolog!(sink, \"??%d\")")
            .unwrap()
            .parse_body::<Nanolog>()
            .is_err());
    }

    #[test]
    fn parse_fail_too_many_args() {
        assert!(
            syn::parse_str::<syn::Macro>("nanolog!(sink, \"??%d\", a, b)")
                .unwrap()
                .parse_body::<Nanolog>()
                .is_err()
        );
    }

    #[test]
    fn parse_success_2_args() {
        assert!(
            syn::parse_str::<syn::Macro>("nanolog!(sink, \"%d %d\", a, b)")
                .unwrap()
                .parse_body::<Nanolog>()
                .is_ok()
        );
    }
}
