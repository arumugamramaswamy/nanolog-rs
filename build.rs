use nanolog_rs_common::{const_fnv1a_hash, Nanolog};
use proc_macro2::TokenStream;
use quote::quote;
use quote::TokenStreamExt;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use syn;
use syn::spanned::Spanned;
use syn::visit::Visit;

fn main() {
    // Get the output directory from cargo
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("source_files.rs");
    let mut file = fs::File::create(&dest_path).unwrap();

    let src_dir = Path::new("src/");
    let files = collect_source_files_into_vec(src_dir);
    let mut v = vec![];
    for f in files.iter() {
        let content = fs::read_to_string(f).unwrap();
        let ast = syn::parse_file(&content).unwrap();
        let mut m = MacroVisitor {
            file_name: f,
            invocations: &mut v,
        };
        m.visit_file(&ast);
    }

    let tokens = quote! {
        pub trait NanologLoggable<Sink: std::io::Write>{
            fn log(self, s: &mut Sink);
        }
    };
    writeln!(file, "{}", tokens).unwrap();

    let mut log_type_extensions = HashSet::new();
    for i in v.iter() {
        log_type_extensions.insert(i.nanolog.get_fmt_string());
    }

    for ext in log_type_extensions {
        let i = quote::format_ident!("Log{}", ext);
        let mut fields = TokenStream::new();
        let mut names = TokenStream::new();
        for (ind, e) in ext.chars().enumerate() {
            let i = quote::format_ident!("field{}", ind);
            fields.extend(match e {
                'D' => quote! { #i: i64, },
                'F' => quote! { #i: f64, },
                _ => unreachable!(),
            });
            names.extend(quote! { #i, });
        }
        let tokens = quote! {
            #[derive(Debug)]
            #[repr(C)]
            pub struct #i<const F: u64, const L: u32>{
                #fields
            }
            impl<const F: u64, const L: u32> #i<F, L>{
                pub fn new(#fields) -> Self{
                    #i{#names}
                }
            }
        };
        writeln!(file, "{}", tokens).unwrap();
    }

    for (log_id, invocation) in v.iter().enumerate() {
        let filename = invocation.file_name.as_str();
        let filehash = const_fnv1a_hash(filename);
        let linenum = invocation.line_num as u32;
        let fmt_literal = invocation.nanolog.fmt_literal.as_str();

        let i = quote::format_ident!("Log{}", invocation.nanolog.get_fmt_string());
        let tokens = quote! {
            impl<Sink: std::io::Write> NanologLoggable<Sink> for #i<#filehash,#linenum>{
                fn log(self, s: &mut Sink){
                    const LOG_ID: usize = #log_id;
                    let struct_bytes = unsafe{
                        let ptr: *const Self = &self;
                        let byte_ptr: *const u8 = ptr.cast();
                        std::slice::from_raw_parts(byte_ptr, std::mem::size_of::<Self>())
                    };
                    s.write(&LOG_ID.to_ne_bytes()).unwrap();
                    s.write(struct_bytes).unwrap();
                    // println!("[{}@{}|ID:{}]{}\n{:?}", #filename, #linenum, LOG_ID, #fmt_literal, struct_bytes);
                }
            }
        };
        writeln!(file, "{}", tokens).unwrap();
    }

    // Tell Cargo to rerun this script if any file in src changes
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");
}

fn log_error(s: &str) {
    println!("cargo::error={s}");
}
fn log_warning(s: &str) {
    println!("cargo::warning={s}");
}

// #[derive(Debug)]
struct LogInvocation {
    nanolog: Nanolog,
    file_name: String,
    line_num: usize,
    s: proc_macro2::Span,
}

impl PartialEq for LogInvocation {
    fn eq(&self, other: &Self) -> bool {
        // Skip comparison of format_str
        self.file_name == other.file_name && self.line_num == other.line_num
    }
}
impl Eq for LogInvocation {}

struct MacroVisitor<'file> {
    file_name: &'file str,
    invocations: &'file mut Vec<LogInvocation>,
}

// should probably replace this with regex matching instead because the following case:
// println!("{}", nanolog!()) doesn't parse correctly into an AST
impl<'ast, 'file> syn::visit::Visit<'ast> for MacroVisitor<'file> {
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        let macro_ident = m.path.get_ident().unwrap();
        if macro_ident != "nanolog" {
            self.visit_path(&m.path);
            return;
        }
        let Ok(n) = m.parse_body::<Nanolog>() else {
            return;
        };

        let span = m.span();
        let start = span.start();

        let invocation = LogInvocation {
            nanolog: n,
            file_name: self.file_name.to_string(),
            line_num: start.line,
            s: span,
        };
        if self
            .invocations
            .iter()
            .find(|i| **i == invocation)
            .is_some()
        {
            log_error(&format!(
                 "Duplicate nanolog invocation found in file {} at line number {}. the following combination must be unique: [filename, linenum]", invocation.file_name, invocation.line_num
             ));
            return;
        }
        self.invocations.push(invocation);
    }
}

fn collect_source_files_into_vec(dir: &Path) -> Vec<String> {
    let mut output = vec![];
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_dir() {
                // Recursively collect files in subdirectories
                output.extend(collect_source_files_into_vec(&path));
            } else if let Some(extension) = path.extension() {
                if extension == "rs" {
                    output.push(path.to_str().unwrap().to_string());
                }
            }
        }
    }
    output
}
