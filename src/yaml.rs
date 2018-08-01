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
			YamlPathElem::Down(ref key) => {
				stack.push(cur);
				match cur {
					Yaml::Hash(ref map) => map.get(&Yaml::String(key.to_string())).unwrap_or(&Yaml::BadValue),
					Yaml::Array(ref arr) => key.parse::<usize>().ok().and_then(|i| arr.get(i)).unwrap_or(&Yaml::BadValue),
					_ => &Yaml::BadValue,
				}
			},
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

pub fn merge(yamls: Vec<Yaml>) -> Yaml {
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
	yamls.into_iter().fold(Yaml::Hash(BTreeMap::new()), |mut ret, cur| { // Flatten all YAMLs into one sequence, and then fold successive ones into earlier ones
		recursive_merge(&mut ret, &cur); // Each fold appends lists, recursively merges objects, and overwrites everything else with the new values
		ret
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use ::std::collections::BTreeMap;
	use ::yaml_rust::YamlLoader;
	#[test]
	fn pathjoin_basic() {
		use YamlPathElem::*;
		assert_eq!(pathjoin(&vec![]), vec![]);
		assert_eq!(pathjoin(&vec![
			&vec![Down("a".to_string()), Down(1.to_string())],
			&vec![Down(0.to_string())],
		]), vec![Down("a".to_string()), Down(1.to_string()), Down(0.to_string())]);
		assert_eq!(pathjoin(&vec![
			&vec![Down("a".to_string()), Down(1.to_string())],
			&vec![Root, Down("b".to_string()), Down("c".to_string())],
			&vec![Up, Down(10.to_string())],
		]), vec![Down("b".to_string()), Down(10.to_string())]);
		assert_eq!(pathjoin(&vec![
			&vec![Root, Up, Up],
			&vec![Down("x".to_string()), Up, Up, Up],
			&vec![Down("y".to_string())],
		]), vec![Down("y".to_string())]);
	}
	#[test]
	fn get_basic() {
		use ::parse::YamlPathElem::*;
		let doc = &YamlLoader::load_from_str("
array:
  - one
  - two
  - three
object:
  nested:
    somewhat: deeply
int: 1
float: 2.0
bool: true
nothing: null
").unwrap()[0];
		assert_eq!(get(doc, &vec![Down("array".to_string())]), &Yaml::Array(vec![Yaml::String("one".to_string()), Yaml::String("two".to_string()), Yaml::String("three".to_string())]));
		assert_eq!(get(doc, &vec![Down("object".to_string()), Down("nested".to_string()), Up, Down("nested".to_string()), Down("somewhat".to_string())]), &Yaml::String("deeply".to_string()));
		assert_eq!(get(doc, &vec![Down("int".to_string())]), &Yaml::Integer(1));
		assert_eq!(get(doc, &vec![Down("float".to_string())]), &Yaml::Real("2.0".to_string()));
		assert_eq!(get(doc, &vec![Down("bool".to_string())]), &Yaml::Boolean(true));
		assert_eq!(get(doc, &vec![Down("nothing".to_string())]), &Yaml::Null);
		assert_eq!(get(doc, &vec![Down("missing".to_string())]), &Yaml::BadValue);
	}
	#[test]
	fn bool_basic() {
		let bad = vec![
			Yaml::Boolean(false),
			Yaml::Null,
			Yaml::BadValue,
			Yaml::Array(vec![]),
			Yaml::Hash(BTreeMap::new())
		];
		let good = vec![
			Yaml::String("".to_string()),
			Yaml::Boolean(true),
			Yaml::Integer(0),
			Yaml::Integer(-1),
			Yaml::Array(vec![Yaml::Boolean(false)]),
			Yaml::Hash(vec![(Yaml::String("".to_string()), Yaml::Null)].into_iter().collect())
		];
		for item in bad.into_iter() { assert!(! bool(&item)); }
		for item in good.into_iter() { assert!(bool(&item)) }
	}
	#[test]
	fn string_basic() {
		assert_eq!(string(&Yaml::String("".to_string()), false).unwrap(), "");
		assert_eq!(string(&Yaml::String("hello".to_string()), false).unwrap(), "hello");
		assert_eq!(string(&Yaml::Null, false).unwrap(), "");
		assert_eq!(string(&Yaml::Integer(-1), false).unwrap(), "-1");
		assert_eq!(string(&Yaml::Real("2.5".to_string()), false).unwrap(), "2.5");
		assert_eq!(string(&Yaml::Boolean(true), false).unwrap(), "true");
		assert_eq!(string(&Yaml::BadValue, true).unwrap(), "");
		assert_eq!(string(&Yaml::Hash(BTreeMap::new()), true).unwrap(), "");
		assert_eq!(string(&Yaml::Array(vec![]), true).unwrap(), "");
		assert!(string(&Yaml::BadValue, false).is_err());
		assert!(string(&Yaml::Hash(BTreeMap::new()), false).is_err());
		assert!(string(&Yaml::Array(vec![]), false).is_err());
	}
	#[test]
	fn merge_basic() {
		let doc1 = YamlLoader::load_from_str("
arr_append:
  - a
  - b
arr_untouched:
  - c
obj_append:
  d: e
  f: g
obj_modify:
  h: i
  j: k
obj_untouched:
  l: m
number: 1
change_type: true
").unwrap().into_iter().next().unwrap();
		let doc2 = YamlLoader::load_from_str("
arr_append:
  - n
  - o
obj_append:
  p: q
  r: s
obj_modify:
  j: t
number: 42
change_type:
  - u
  - v
new_object:
  w: 1
  x: null
").unwrap().into_iter().next().unwrap();
		let doc_res = YamlLoader::load_from_str("
arr_append:
  - a
  - b
  - n
  - o
arr_untouched:
  - c
obj_append:
  d: e
  f: g
  p: q
  r: s
obj_modify:
  h: i
  j: t
obj_untouched:
  l: m
number: 42
change_type:
  - u
  - v
new_object:
  w: 1
  x: null
").unwrap().into_iter().next().unwrap();
		assert_eq!(merge(vec![doc1, doc2]), doc_res);
	}
}
