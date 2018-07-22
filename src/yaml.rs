use ::yaml_rust::Yaml;
use ::std::collections::BTreeMap;
use ::std::collections::btree_map::Entry;
use ::parse::*;
use ::errors::*;

pub fn pathjoin<'a>(paths: &[&YamlPath]) -> YamlPath {
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

pub fn get<'a>(root: &'a Yaml, path: &YamlPath) -> &'a Yaml {
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

pub fn bool(yaml: &Yaml) -> bool {
	match yaml {
		Yaml::BadValue | Yaml::Null | Yaml::Boolean(false) => false,
		Yaml::Array(ref a) => ! a.is_empty(),
		Yaml::Hash(ref h) => ! h.is_empty(),
		_ => true,
	}
}

pub fn string(yaml: &Yaml, ignore: bool) -> Result<String> {
	match yaml {
		Yaml::Real(x) => Ok(x.to_string()),
		Yaml::Integer(x) => Ok(x.to_string()),
		Yaml::String(x) => Ok(x.to_string()),
		Yaml::Boolean(x) => Ok(x.to_string()),
		Yaml::Null => Ok("".to_string()),
		_ => if ignore { Ok("".to_string()) } else { Err(Error::from("Can't stringify type")) }, // TODO This error message (and a lot of others) needs to be better
	}
}

pub fn merge(yamls: Vec<Option<Vec<Yaml>>>) -> Yaml {
	fn recursive_merge(acc: &mut Yaml, cur: &Yaml) { // TODO Is there a way to do this without all the clones?  We should take ownership of cur so we can butcher it for its pieces.
		match (acc, cur) {
			(Yaml::Array(orig), Yaml::Array(new)) => orig.extend(new.iter().cloned()),
			(Yaml::Hash(orig), Yaml::Hash(new)) =>
				for (k, v) in new.iter() {
					match orig.entry(k.clone()) {
						Entry::Vacant(entry) => { entry.insert(v.clone()); },
						Entry::Occupied(mut entry) => { recursive_merge(entry.get_mut(), v); },
					}
				}
			(orig, new) => *orig = new.clone(),
		}
	}
	yamls.into_iter().filter_map(|yaml| yaml).flat_map(|yaml| yaml.into_iter()).fold(Yaml::Hash(BTreeMap::new()), |mut ret, cur| { // Flatten all YAMLs into one sequence, and then fold successive ones into earlier ones
		recursive_merge(&mut ret, &cur); // Each fold appends lists, recursively merges objects, and overwrites everything else with the new values
		ret
	})
}
