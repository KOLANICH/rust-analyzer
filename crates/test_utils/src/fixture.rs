//! Defines `Fixture` -- a convenient way to describe the initial state of
//! rust-analyzer database from a single string.
//!
//! Fixtures are strings containing rust source code with optional metadata.
//! A fixture without metadata is parsed into a single source file.
//! Use this to test functionality local to one file.
//!
//! Simple Example:
//! ```
//! r#"
//! fn main() {
//!     println!("Hello World")
//! }
//! "#
//! ```
//!
//! Metadata can be added to a fixture after a `//-` comment.
//! The basic form is specifying filenames,
//! which is also how to define multiple files in a single test fixture
//!
//! Example using two files in the same crate:
//! ```
//! "
//! //- /main.rs
//! mod foo;
//! fn main() {
//!     foo::bar();
//! }
//!
//! //- /foo.rs
//! pub fn bar() {}
//! "
//! ```
//!
//! Example using two crates with one file each, with one crate depending on the other:
//! ```
//! r#"
//! //- /main.rs crate:a deps:b
//! fn main() {
//!     b::foo();
//! }
//! //- /lib.rs crate:b
//! pub fn b() {
//!     println!("Hello World")
//! }
//! "#
//! ```
//!
//! Metadata allows specifying all settings and variables
//! that are available in a real rust project:
//! - crate names via `crate:cratename`
//! - dependencies via `deps:dep1,dep2`
//! - configuration settings via `cfg:dbg=false,opt_level=2`
//! - environment variables via `env:PATH=/bin,RUST_LOG=debug`
//!
//! Example using all available metadata:
//! ```
//! "
//! //- /lib.rs crate:foo deps:bar,baz cfg:foo=a,bar=b env:OUTDIR=path/to,OTHER=foo
//! fn insert_source_code_here() {}
//! "
//! ```

use rustc_hash::FxHashMap;
use stdx::{lines_with_ends, split_once, trim_indent};

#[derive(Debug, Eq, PartialEq)]
pub struct Fixture {
    pub path: String,
    pub text: String,
    pub krate: Option<String>,
    pub deps: Vec<String>,
    pub cfg_atoms: Vec<String>,
    pub cfg_key_values: Vec<(String, String)>,
    pub edition: Option<String>,
    pub env: FxHashMap<String, String>,
    pub introduce_new_source_root: bool,
}

pub struct MiniCore {
    activated_flags: Vec<String>,
    valid_flags: Vec<String>,
}

impl Fixture {
    /// Parses text which looks like this:
    ///
    ///  ```not_rust
    ///  //- some meta
    ///  line 1
    ///  line 2
    ///  //- other meta
    ///  ```
    ///
    /// Fixture can also start with a minicore declaration:
    ///
    /// ```
    /// //- minicore: sized
    /// ```
    ///
    /// That will include a subset of `libcore` into the fixture, see
    /// `minicore.rs` for what's available.
    pub fn parse(ra_fixture: &str) -> (Option<MiniCore>, Vec<Fixture>) {
        let fixture = trim_indent(ra_fixture);
        let mut fixture = fixture.as_str();
        let mut mini_core = None;
        let mut res: Vec<Fixture> = Vec::new();

        if fixture.starts_with("//- minicore:") {
            let first_line = fixture.split('\n').next().unwrap().to_owned() + "\n";
            
            mini_core = Some(MiniCore::parse(&first_line));
            fixture = &fixture[first_line.len()..];
        }

        let default = if fixture.contains("//-") { None } else { Some("//- /main.rs") };

        for (ix, line) in default.into_iter().chain(lines_with_ends(&fixture)).enumerate() {
            if line.contains("//-") {
                assert!(
                    line.starts_with("//-"),
                    "Metadata line {} has invalid indentation. \
                     All metadata lines need to have the same indentation.\n\
                     The offending line: {:?}",
                    ix,
                    line
                );
            }

            if line.starts_with("//-") {
                let meta = Fixture::parse_meta_line(line);
                res.push(meta)
            } else {
                if line.starts_with("// ")
                    && line.contains(':')
                    && !line.contains("::")
                    && line.chars().all(|it| !it.is_uppercase())
                {
                    panic!("looks like invalid metadata line: {:?}", line)
                }

                if let Some(entry) = res.last_mut() {
                    entry.text.push_str(line);
                }
            }
        }

        (mini_core, res)
    }

    //- /lib.rs crate:foo deps:bar,baz cfg:foo=a,bar=b env:OUTDIR=path/to,OTHER=foo
    fn parse_meta_line(meta: &str) -> Fixture {
        assert!(meta.starts_with("//-"));
        let meta = meta["//-".len()..].trim();
        let components = meta.split_ascii_whitespace().collect::<Vec<_>>();

        let path = components[0].to_string();
        assert!(path.starts_with('/'), "fixture path does not start with `/`: {:?}", path);

        let mut krate = None;
        let mut deps = Vec::new();
        let mut edition = None;
        let mut cfg_atoms = Vec::new();
        let mut cfg_key_values = Vec::new();
        let mut env = FxHashMap::default();
        let mut introduce_new_source_root = false;
        for component in components[1..].iter() {
            let mut splitted = component.split(':');
            let key: &str;
            let value: &str;
            match splitted.next() {
                Some(key_part) => {
                    match splitted.next() {
                        Some(value_part) => {
                            key = key_part;
                            value = value_part;
                        }
                        None => {panic!("invalid meta line: {:?}", meta);}
                   }
                }
                None => {panic!("invalid meta line: {:?}", meta);}
            }
            match key {
                "crate" => krate = Some(value.to_string()),
                "deps" => deps = value.split(',').map(|it| it.to_string()).collect(),
                "edition" => edition = Some(value.to_string()),
                "cfg" => {
                    for entry in value.split(',') {
                        match split_once(entry, '=') {
                            Some((k, v)) => cfg_key_values.push((k.to_string(), v.to_string())),
                            None => cfg_atoms.push(entry.to_string()),
                        }
                    }
                }
                "env" => {
                    for key in value.split(',') {
                        if let Some((k, v)) = split_once(key, '=') {
                            env.insert(k.into(), v.into());
                        }
                    }
                }
                "new_source_root" => introduce_new_source_root = true,
                _ => panic!("bad component: {:?}", component),
            }
        }

        Fixture {
            path,
            text: String::new(),
            krate,
            deps,
            cfg_atoms,
            cfg_key_values,
            edition,
            env,
            introduce_new_source_root,
        }
    }
}

impl MiniCore {
    fn has_flag(&self, flag: &str) -> bool {
        self.activated_flags.iter().any(|it| it == flag)
    }

    #[track_caller]
    fn assert_valid_flag(&self, flag: &str) {
        if !self.valid_flags.iter().any(|it| it == flag) {
            panic!("invalid flag: {:?}, valid flags: {:?}", flag, self.valid_flags);
        }
    }

    fn parse(line: &str) -> MiniCore {
        let mut res = MiniCore { activated_flags: Vec::new(), valid_flags: Vec::new() };

        let line = line.strip_prefix("//- minicore:").unwrap().trim();
        for entry in line.split(", ") {
            if res.has_flag(entry) {
                panic!("duplicate minicore flag: {:?}", entry)
            }
            res.activated_flags.push(entry.to_string())
        }

        res
    }

    /// Strips parts of minicore.rs which are flagged by inactive flags.
    ///
    /// This is probably over-engineered to support flags dependencies.
    pub fn source_code(mut self) -> String {
        let mut buf = String::new();
        let raw_mini_core = include_str!("./minicore.rs");
        let mut lines = raw_mini_core.split('\n');

        let mut parsing_flags = false;
        let mut implications = Vec::new();

        // Parse `//!` preamble and extract flags and dependencies.
        for line in lines.by_ref() {
            let line = match line.strip_prefix("//!") {
                Some(it) => it.to_string() + "\n",
                None => {
                    assert!(line.trim().is_empty());
                    break;
                }
            };

            if parsing_flags {
                let flag: String;
                let deps: String;
                let mut splitted = line.split(':');
                match splitted.next() {
                    Some(flag_v) => {
                        flag = flag_v.to_owned();
                        match splitted.next() {
                            Some(deps_v) => {
                                deps = deps_v.to_owned();
                            },
                            None => {
                                deps = "".to_owned();
                            }
                        }
                    },
                    None => {
                        flag = "".to_owned();
                        deps = "".to_owned();
                    }
                }
                let flag = flag.trim();
                self.valid_flags.push(flag.to_string().to_owned());
                for dep in deps.split(", ") {
                    let dep = dep.trim();
                    if !dep.is_empty() {
                        self.assert_valid_flag(&dep);
                        implications.push((flag.to_owned(), dep.to_owned()));
                    }
                }
            }

            if line.contains("Available flags:") {
                parsing_flags = true;
            }
        }

        for flag in &self.activated_flags {
            self.assert_valid_flag(flag);
        }

        // Fixed point loop to compute transitive closure of flags.
        loop {
            let mut changed = false;
            for (u, v) in implications.iter() {
                if self.has_flag(&u) && !self.has_flag(&v) {
                    self.activated_flags.push(v.to_string());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        let mut active_regions = Vec::new();
        let mut seen_regions = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if let Some(region) = trimmed.strip_prefix("// region:") {
                active_regions.push(region);
                continue;
            }
            if let Some(region) = trimmed.strip_prefix("// endregion:") {
                let prev = active_regions.pop().unwrap();
                assert_eq!(prev, region);
                continue;
            }

            let mut line_region = false;
            if let Some(idx) = trimmed.find("// :") {
                line_region = true;
                active_regions.push(&trimmed[idx + "// :".len()..]);
            }

            let mut keep = true;
            for &region in &active_regions {
                assert!(
                    !region.starts_with(' '),
                    "region marker starts with a space: {:?}",
                    region
                );
                self.assert_valid_flag(region);
                seen_regions.push(region);
                keep &= self.has_flag(region);
            }

            if keep {
                buf.push_str(line)
            }
            if line_region {
                active_regions.pop().unwrap();
            }
        }

        for flag in &self.valid_flags {
            if !seen_regions.iter().any(|it| it == flag) {
                panic!("unused minicore flag: {:?}", flag);
            }
        }
        format!("{}", buf);
        buf
    }
}

#[test]
#[should_panic]
fn parse_fixture_checks_further_indented_metadata() {
    Fixture::parse(
        r"
        //- /lib.rs
          mod bar;

          fn foo() {}
          //- /bar.rs
          pub fn baz() {}
          ",
    );
}

#[test]
fn parse_fixture_gets_full_meta() {
    let (mini_core, parsed) = Fixture::parse(
        r#"
//- minicore: coerce_unsized
//- /lib.rs crate:foo deps:bar,baz cfg:foo=a,bar=b,atom env:OUTDIR=path/to,OTHER=foo
mod m;
"#,
    );
    assert_eq!(mini_core.unwrap().activated_flags, vec!["coerce_unsized".to_string()]);
    assert_eq!(1, parsed.len());

    let meta = &parsed[0];
    assert_eq!("mod m;\n", meta.text);

    assert_eq!("foo", meta.krate.as_ref().unwrap());
    assert_eq!("/lib.rs", meta.path);
    assert_eq!(2, meta.env.len());
}
