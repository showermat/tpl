#[macro_use] extern crate error_chain;
#[macro_use] extern crate clap;
#[macro_use] extern crate nom;
extern crate yaml_rust;

#[cfg(test)] mod tests;
mod parse;

use std::fs::File;
use std::io::Read;
use std::io::BufReader;
use yaml_rust::Yaml;
use parse::*;

mod errors { error_chain!{} }
use errors::*;

// TODO
// Split into multiple files; add TESTS and DOCS
// https://stackoverflow.com/questions/46876879/how-do-i-create-a-streaming-parser-in-nom
// Error if there are unmatched conds (missing {{/}}s) rather than implicitly closing them at the end (related to next line)
// Allow text to be included after the / in EndSub.  Either ignore it or require it to match the start text
// Options to collapse whitespace?
// All the error messages need to be a lot nicer
// Allow escaping so that {{ can appear in the document if someone really needs it to

fn read_file(path: &str) -> Result<String> {
	let mut ret = String::new();
	BufReader::new(File::open(path).chain_err(|| format!("Failed to open {}", path))?)
		.read_to_string(&mut ret).chain_err(|| format!("Failed to read from {}", path))?;
	Ok(ret)
}

fn yaml_pathjoin<'a>(paths: &[&YamlPath]) -> YamlPath {
	let mut ret = vec![];
	for elem in paths.iter().flat_map(|x| x.iter()) {
		match elem {
			YamlPathElem::Up => { ret.pop(); }, // It's okay if we pop an empty Vec // TODO Is this the best way to ignore the return value?
			YamlPathElem::Root => ret.clear(),
			_ => ret.push(elem.clone()), // TODO Is this clone necessary?
		};
	}
	ret
}

fn yaml_get<'a>(root: &'a Yaml, path: &YamlPath) -> &'a Yaml {
	let mut cur = root;
	let mut stack = vec![];
	for elem in path.iter() {
		cur = match elem {
			YamlPathElem::DownObject(ref key) => { stack.push(cur); &cur[&key[..]] },
			YamlPathElem::DownArray(key) => { stack.push(cur); &cur[*key as usize] },
			YamlPathElem::Up => stack.pop().unwrap_or(root),
			YamlPathElem::Root => root,
		};
	}
	cur
}

fn yaml_bool(yaml: &Yaml) -> bool {
	match yaml {
		Yaml::BadValue | Yaml::Null | Yaml::Boolean(false) => false,
		Yaml::Array(ref a) => ! a.is_empty(),
		Yaml::Hash(ref h) => ! h.is_empty(),
		_ => true,
	}
}

fn yaml_string(yaml: &Yaml, ignore: bool) -> Result<String> {
	match yaml {
		Yaml::Real(x) => Ok(x.to_string()),
		Yaml::Integer(x) => Ok(x.to_string()),
		Yaml::String(x) => Ok(x.to_string()),
		Yaml::Boolean(x) => Ok(x.to_string()),
		Yaml::Null => Ok("".to_string()),
		_ => if ignore { Ok("".to_string()) } else { Err(Error::from("Can't stringify type")) }, // TODO This error message (and a lot of others) needs to be better
	}
}

fn render(values: &Yaml, tree: &[Node], context: &YamlPath, ignore: bool) -> Result<String> {
	let mut ret = "".to_string();
	for node in tree {
		let cur = match node {
			Node::Literal(ref s) => s.to_string(),
			Node::DirectSub(ref path) => yaml_string(yaml_get(values, &yaml_pathjoin(&vec![context, path][..])), ignore).chain_err(|| "Couldn't stringify value")?,
			Node::CondSub(ref path, direct, ref children) => {
				let abspath = &yaml_pathjoin(&vec![context, path][..]);
				let target = yaml_get(values, abspath);
				if yaml_bool(target) && *direct {
					let render_child = |child| render(values, children, &yaml_pathjoin(&vec![abspath, &vec![child]]), ignore);
					match target {
						// TODO Don't use as_str() (this will require doing something about the non-string key case); try to pull out common parts of these lines
						Yaml::Hash(ref contents) => contents.keys().map(|k| match k {
							Yaml::String(ref s) => render_child(YamlPathElem::DownObject(s.to_string())),
							_ => Err(Error::from("All YAML keys must be strings")),
						}).collect::<Result<String>>()?,
						Yaml::Array(ref contents) => (0..contents.len() as i64).into_iter().map(|i| render_child(YamlPathElem::DownArray(i))).collect::<Result<String>>()?,
						_ => render(values, children, abspath, ignore)?,
					}
				}
				else if ! yaml_bool(target) && ! *direct {
					render(values, children, abspath, ignore)?
				}
				else { "".to_string() }
			},
			Node::KeySub(n) => match context.iter().rev().nth(*n as usize).ok_or(Error::from("No key in this context"))? {
				YamlPathElem::DownObject(ref k) => k.to_string(),
				YamlPathElem::DownArray(i) => i.to_string(),
				_ => bail!("KeySub attempted on unexpected path element"),
			},
		};
		ret.push_str(&cur);
	}
	Ok(ret)
}

fn matching_delim(open: &str) -> String { // TODO Not Unicode-aware.  Is it practical?
	fn flip(c: char) -> char {
		match c {
			'(' => ')', '[' => ']', '{' => '}', '<' => '>',
			')' => '(', ']' => '[', '}' => '{', '>' => '<',
			x => x,
		}
	}
	open.chars().map(flip).collect()
}

fn run() -> Result<()> {
	let args = clap_app!(tpl =>
		(about: "Simple multi-purpose template engine")
		(@arg input: * index(1) "File to be templated")
		(@arg values: -f [file] "YAML file of template values")
		(@arg delim_a: -A [delim] "Opening delimiter for tags (default \"{{\")")
		(@arg delim_b: -B [delim] "Closing delimiter for tags (default \"}}\")")
		(@arg ignore: -i "Ignore rather than erroring on invalid subtitutions")
	).get_matches();

	let input = read_file(args.value_of("input").unwrap()).chain_err(|| "Failed to get input")?; // This unwrap is safe
	let delim_open = args.value_of("delim_a").unwrap_or("{{").to_string();
	let delim_close = args.value_of("delim_b").map(|x| x.to_string()).unwrap_or(matching_delim(&delim_open));
	let parsed = parse_string(&input, &delim_open, &delim_close)?; // Returns (yaml, tree)
	let values: Vec<Yaml> = match args.value_of("values") {
		Some(fname) => yaml_rust::YamlLoader::load_from_str(&read_file(fname).chain_err(|| "Failed to read values file")?).chain_err(|| "Failed to parse values file")?,
		None => parsed.0.ok_or(Error::from("Values are required either inline in the input or using the values flag"))?, // Could we merge them if both exist?
	};
	let values = &values[0]; // TODO What should we do if there are multiple streams in the file?  Ignore them?
	print!("{}", render(values, &parsed.1, &vec![], args.is_present("ignore"))?);
	Ok(())
}

quick_main!(run);
