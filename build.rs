use nanolog_rs_common::{const_fnv1a_hash, Nanolog};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
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
        use std::io::Write;
        use nanolog_rs_common::compression::NibbleNibble;

        pub trait Compressable{
            fn compress(&self, writer: &mut impl Write);
        }

        impl Compressable for (u64, u64){
            fn compress(&self, writer: &mut impl Write){
                const MAX_SIZE: usize = 8 + 8 + 1;

                let mut bytes = [0_u8; MAX_SIZE];

                let nb = NibbleNibble::from(*self);
                bytes[0] = nb.0;

                let (lower_size, upper_size) = nb.get_num_bytes();
                let lower_size = lower_size.map(|v|v.get()).unwrap_or(8);
                let upper_size = upper_size.map(|v|v.get()).unwrap_or(8);
                // bytes[1..1 + lower_size].copy_from_slice(&self.0.to_le_bytes()[..lower_size]);
                // bytes[1 + lower_size..1 + lower_size + upper_size].copy_from_slice(&self.1.to_le_bytes()[..upper_size]);

                writer.write(&[nb.0]).unwrap();
                writer.write(&self.0.to_le_bytes()[..lower_size]).unwrap();
                writer.write(&self.1.to_le_bytes()[..upper_size]).unwrap();
                // writer.write(&bytes[..upper_size as usize + lower_size as usize + 1]).unwrap();
            }
        }

        pub trait NanologLoggable<const F: u64, const L: u32>: Compressable{
            fn log(self, logger: &mut impl ::nanolog_rs_common::nanolog_logger::Logger);
        }
    };
    writeln!(file, "{}", tokens).unwrap();

    let mut log_type_extensions = HashSet::new();
    for i in v.iter() {
        log_type_extensions.insert(i.nanolog.get_log_type_suffix());
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
            pub struct #i{
                #fields
            }
            impl #i{
                pub fn new(#fields) -> Self{
                    #i{#names}
                }
            }

            impl Compressable for #i{
                fn compress(&self, writer: &mut impl Write){
                    todo!()
                }
            }
        };
        writeln!(file, "{}", tokens).unwrap();
    }
    let mut log_id_map = TokenStream::new();

    for (log_id, invocation) in v.iter().enumerate() {
        let filename = invocation.file_name.as_str();
        let filehash = const_fnv1a_hash(filename);
        let linenum = invocation.line_num as u32;
        let fmt_literal = invocation.nanolog.fmt_literal.as_str();

        let log_id_u64 = log_id as u64;

        let i = quote::format_ident!("Log{}", invocation.nanolog.get_log_type_suffix());
        let tokens = quote! {
            impl NanologLoggable<#filehash,#linenum> for #i{
                fn log(self, logger: &mut impl ::nanolog_rs_common::nanolog_logger::Logger){
                    const LOG_ID: u64 = #log_id_u64;

                    let timestamp = ::nanolog_rs_common::get_rdtsc_time();
                    // let timestamp = ::nanolog_rs_common::get_monotonic_time_micros();
                    // let timestamp = ::nanolog_rs_common::system_time_to_micros(::std::time::SystemTime::now());

                    logger.write(&LOG_ID.to_ne_bytes());

                    logger.write(&timestamp.to_ne_bytes());

                    if (std::mem::size_of::<Self>() > 0){
                        let struct_bytes = unsafe{
                            let ptr: *const Self = &self;
                            let byte_ptr: *const u8 = ptr.cast();
                            std::slice::from_raw_parts(byte_ptr, std::mem::size_of::<Self>())
                        };
                        logger.write(struct_bytes);
                    }

                    logger.commit_write();
                }
            }
        };

        log_id_map.extend(quote! {#fmt_literal, });

        writeln!(file, "{}", tokens).unwrap();
    }
    let n = v.len();
    writeln!(
        file,
        "const LOG_LITERALS: [&'static str; {n}] = [{}];",
        log_id_map
    )
    .unwrap();

    let mut log_id_cases = TokenStream::new();

    for (log_id, invocation) in v.iter().enumerate() {
        let log_id_u64 = log_id as u64;

        let i = quote::format_ident!("Log{}", invocation.nanolog.get_log_type_suffix());
        log_id_cases.extend(quote! {
            #log_id_u64 => {
                const LOG_SIZE: usize = std::mem::size_of::<crate::nanolog_internal::#i>();

                (log_id, timestamp).compress(out);

                out.write(&buf[consumed..consumed + LOG_SIZE]).unwrap();
                // TODO: impl compression for general types
                // let log_type = unsafe{&*(buf[consumed..consumed + LOG_SIZE].as_ptr() as *const crate::nanolog_internal::#i)};
                // log_type.compress(out);
                // println!("[{}] Fmt literal: {}, log_type: {:?}", timestamp, LOG_LITERALS[#log_id], log_type);
                consumed += LOG_SIZE;
            }
        });
    }

    let decode_buf = quote! {
        pub fn decode_buf(out: &mut impl Write, start_instant: &::std::time::Instant, buf: &[u8]) {
            // TODO: this is not the correct number of bytes - this is the number of bytes before
            // compression
            out.write(&buf.len().to_le_bytes());
            let mut consumed = 0;
            while !buf[consumed..].is_empty() {
                let mut bytes = [0u8; 8];

                bytes.copy_from_slice(&buf[consumed..consumed + 8]);
                consumed += 8;
                let log_id = u64::from_le_bytes(bytes);

                bytes.copy_from_slice(&buf[consumed..consumed + 8]);
                consumed += 8;
                let timestamp = u64::from_le_bytes(bytes);

                match log_id {
                    #log_id_cases
                    _ => panic!("unknown log id"),
                }
            }
        }
    };
    writeln!(file, "{}", decode_buf).unwrap();

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
