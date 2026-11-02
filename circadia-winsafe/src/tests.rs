use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use syn::visit::Visit;

#[derive(Default)]
struct UnsafeSites(usize);

impl<'ast> Visit<'ast> for UnsafeSites {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.0 += 1;
        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_signature(&mut self, node: &'ast syn::Signature) {
        self.0 += usize::from(node.unsafety.is_some());
        syn::visit::visit_signature(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        self.0 += usize::from(node.unsafety.is_some());
        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.0 += usize::from(node.unsafety.is_some());
        syn::visit::visit_item_trait(self, node);
    }

    fn visit_item_foreign_mod(&mut self, node: &'ast syn::ItemForeignMod) {
        self.0 += usize::from(node.unsafety.is_some());
        syn::visit::visit_item_foreign_mod(self, node);
    }

    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        self.0 += usize::from(node.path().is_ident("unsafe"));
        if let syn::Meta::List(list) = &node.meta {
            self.0 += unsafe_tokens(list.tokens.clone());
        }
        syn::visit::visit_attribute(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        self.0 += unsafe_tokens(node.tokens.clone());
        syn::visit::visit_macro(self, node);
    }
}

fn unsafe_tokens(tokens: proc_macro2::TokenStream) -> usize {
    tokens
        .into_iter()
        .map(|token| match token {
            proc_macro2::TokenTree::Ident(ident) => usize::from(ident == "unsafe"),
            proc_macro2::TokenTree::Group(group) => unsafe_tokens(group.stream()),
            proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => 0,
        })
        .sum()
}

fn count(directory: &Path, root: &Path, sites: &mut BTreeMap<String, usize>) {
    for entry in fs::read_dir(directory).expect("readable source directory") {
        let path = entry.expect("readable source entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                count(&path, root, sites);
            }
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("readable source file");
        let mut visitor = UnsafeSites::default();
        visitor.visit_file(&syn::parse_file(&source).expect("parsable source file"));
        if visitor.0 > 0 {
            let name = path
                .strip_prefix(root)
                .expect("source inside the crate")
                .to_string_lossy()
                .replace('\\', "/");
            sites.insert(name, visitor.0);
        }
    }
}

fn listed(readme: &str) -> BTreeMap<String, usize> {
    readme
        .lines()
        .skip_while(|line| *line != "## Unsafe inventory")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .filter_map(|line| {
            let (file, rest) = line.strip_prefix("| `")?.split_once("` |")?;
            let sites = rest.trim().trim_end_matches('|').trim().parse().ok()?;
            Some((file.to_owned(), sites))
        })
        .fold(BTreeMap::new(), |mut rows, (file, sites)| {
            assert!(
                rows.insert(file.clone(), sites).is_none(),
                "README inventory lists {file} twice"
            );
            rows
        })
}

#[test]
fn readme_inventory_matches_the_unsafe_sites_in_the_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut actual = BTreeMap::new();
    count(root, root, &mut actual);
    let readme = fs::read_to_string(root.join("README.md")).expect("readable README");
    let table = actual
        .iter()
        .fold(String::new(), |mut table, (file, sites)| {
            writeln!(table, "| `{file}` | {sites} |").expect("writable table");
            table
        });
    assert_eq!(
        listed(&readme),
        actual,
        "replace the rows under \"## Unsafe inventory\" in README.md with:\n{table}"
    );
}
