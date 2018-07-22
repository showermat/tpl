#[macro_use] extern crate error_chain;
#[macro_use] extern crate clap;
#[macro_use] extern crate nom;
extern crate yaml_rust;

#[cfg(test)] mod tests;
mod parse;
mod yaml;

use std::fs::File;
use std::io::Read;
use std::io::BufReader;
use std::collections::BTreeMap;
use yaml_rust::Yaml;
use parse::*;

mod errors { error_chain!{} }
use errors::*;

// TODO
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

fn render(values: &Yaml, tree: &[Node], context: &YamlPath, ignore: bool) -> Result<String> {
	let mut ret = "".to_string();
	for node in tree {
		let cur = match node {
			Node::Literal(ref s) => s.to_string(),
			Node::DirectSub(ref path) => yaml::string(yaml::get(values, &yaml::pathjoin(&vec![context, path][..])), ignore).chain_err(|| "Couldn't stringify value")?,
			Node::CondSub(ref path, direct, ref children) => {
				let abspath = &yaml::pathjoin(&vec![context, path][..]);
				let target = yaml::get(values, abspath);
				if yaml::bool(target) && *direct {
					let render_child = |child| render(values, children, &yaml::pathjoin(&vec![abspath, &vec![child]]), ignore);
					match target {
						Yaml::Hash(ref contents) => contents.keys().map(|k| match k {
							Yaml::String(ref s) => render_child(YamlPathElem::DownObject(s.to_string())),
							_ => Err(Error::from("All YAML keys must be strings")),
						}).collect::<Result<String>>()?,
						Yaml::Array(ref contents) => (0..contents.len() as i64).into_iter().map(|i| render_child(YamlPathElem::DownArray(i))).collect::<Result<String>>()?,
						_ => render(values, children, abspath, ignore)?,
					}
				}
				else if ! yaml::bool(target) && ! *direct { render(values, children, abspath, ignore)?  }
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

struct ParseArgs {
	pub open: String,
	pub close: String,
	pub ignore: bool,
}

impl ParseArgs {
	fn from_yaml(yaml: &mut Yaml) -> Result<Self> {
		if let Yaml::Hash(h) = yaml {
			if let Yaml::Hash(m) = h.entry(Yaml::String("_meta".to_string())).or_insert(Yaml::Hash(BTreeMap::new())) {
				let open = match m.entry(Yaml::String("open".to_string())).or_insert(Yaml::String("{{".to_string())) {
					Yaml::String(s) => s.clone(),
					_ => bail!("_meta.open must be a string"),
				};
				let close = match m.entry(Yaml::String("close".to_string())).or_insert(Yaml::String(matching_delim(&open))) {
					Yaml::String(s) => s.clone(),
					_ => bail!("_meta.close must be a string"),
				};
				let ignore = match m.entry(Yaml::String("ignore".to_string())).or_insert(Yaml::Boolean(false)) {
					Yaml::Boolean(b) => b,
					_ => bail!("_meta.ignore must be a boolean"),
				};
				Ok(ParseArgs { open: open.to_string(), close: close.to_string(), ignore: *ignore })
			}
			else { bail!("_meta must be an object"); }
		}
		else { bail!("Top-level YAML must be an object"); }
	}
}

fn run() -> Result<()> {
	let args = clap_app!(tpl =>
		(about: "Simple multi-purpose template engine")
		(@arg input: * index(1) "File to be templated")
		(@arg values: -f [file] "YAML file of template values")
	).get_matches();

	let input = read_file(args.value_of("input").unwrap()).chain_err(|| "Failed to get input")?; // This unwrap is safe
	let cli_values = match args.value_of("values") { // TODO I'd like to convert this match to a map, but that's hard to do with the try operators....
		Some(fname) => Some(yaml_rust::YamlLoader::load_from_str(&read_file(fname).chain_err(|| "Failed to read values file")?).chain_err(|| "Failed to parse values file")?),
		None => None,
	};
	let mut parser = Parser::new(&input);
	let mut values = yaml::merge(vec![parser.get_yaml()?, cli_values]);
	let pargs = ParseArgs::from_yaml(&mut values).chain_err(|| "Error parsing template arguments")?;
	print!("{}", render(&values, &parser.get_tpl(&pargs.open, &pargs.close)?, &vec![], pargs.ignore)?);
	Ok(())
}

quick_main!(run);
