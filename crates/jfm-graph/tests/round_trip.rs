use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use jfm_graph::SurrealGraphStore;
use jfm_model::{
    ClassInfo, ClassKind, Endpoint, Fqn, HttpVerb, MethodInfo, ParamInfo, ParamSource,
    ProjectIndex, TypeRef,
};
use tempfile::tempdir;

fn type_ref(raw: &str) -> TypeRef {
    TypeRef {
        raw: raw.into(),
        generics: vec![],
    }
}

fn sample_index() -> ProjectIndex {
    let class_fqn = Fqn("com.example.UserController".into());
    let method_fqn = Fqn("com.example.UserController.getUser".into());

    let method = MethodInfo {
        fqn: method_fqn.clone(),
        name: "getUser".into(),
        params: vec![ParamInfo {
            name: "id".into(),
            ty: "Long".into(),
            source: ParamSource::Path,
            annotations: Vec::new(),
            validation: Vec::new(),
        }],
        return_type: type_ref("User"),
        annotations: vec!["@GetMapping".into()],
        body: vec![],
        locals: HashMap::new(),
        file: PathBuf::from("src/main/java/com/example/UserController.java"),
        line: 42,
    };

    let class = ClassInfo {
        fqn: class_fqn.clone(),
        simple_name: "UserController".into(),
        package: "com.example".into(),
        imports: HashMap::new(),
        kind: ClassKind::Class,
        annotations: vec!["@RestController".into()],
        validation: Vec::new(),
        extends: vec![],
        implements: vec![],
        fields: vec![],
        methods: vec![method],
        file: PathBuf::from("src/main/java/com/example/UserController.java"),
        line: 12,
    };

    let endpoint = Endpoint {
        verb: HttpVerb::Get,
        path: "/users/{id}".into(),
        handler_fqn: method_fqn,
        file: PathBuf::from("src/main/java/com/example/UserController.java"),
        line: 42,
    };

    let mut classes = HashMap::new();
    classes.insert(class_fqn.clone(), class);

    let mut by_simple_name = HashMap::new();
    by_simple_name.insert("UserController".into(), vec![class_fqn]);

    ProjectIndex {
        classes,
        by_simple_name,
        endpoints: vec![endpoint],
    }
}

#[test]
fn round_trip_empty_index() -> Result<()> {
    let tmp = tempdir()?;
    let store = SurrealGraphStore::open(tmp.path().join("db"))?;

    assert!(store.load_project_index()?.is_none());

    let index = ProjectIndex::default();
    store.save_project_index(&index)?;

    let loaded = store.load_project_index()?.expect("expected cached index");
    assert!(loaded.classes.is_empty());
    assert!(loaded.endpoints.is_empty());
    Ok(())
}

#[test]
fn round_trip_populated_index() -> Result<()> {
    let tmp = tempdir()?;
    let store = SurrealGraphStore::open(tmp.path().join("db"))?;

    let index = sample_index();
    store.save_project_index(&index)?;

    let loaded = store.load_project_index()?.expect("expected cached index");
    assert_eq!(loaded.classes.len(), 1);
    assert_eq!(loaded.endpoints.len(), 1);

    let class_fqn = Fqn("com.example.UserController".into());
    let class = loaded.classes.get(&class_fqn).expect("class missing");
    assert_eq!(class.simple_name, "UserController");
    assert_eq!(class.methods.len(), 1);
    assert_eq!(class.methods[0].name, "getUser");
    assert_eq!(class.methods[0].params.len(), 1);
    assert_eq!(class.methods[0].params[0].source, ParamSource::Path);

    let endpoint = &loaded.endpoints[0];
    assert_eq!(endpoint.verb, HttpVerb::Get);
    assert_eq!(endpoint.path, "/users/{id}");

    assert_eq!(
        loaded.by_simple_name.get("UserController"),
        Some(&vec![class_fqn])
    );
    Ok(())
}

#[test]
fn save_overwrites_existing_record() -> Result<()> {
    let tmp = tempdir()?;
    let store = SurrealGraphStore::open(tmp.path().join("db"))?;

    store.save_project_index(&ProjectIndex::default())?;
    store.save_project_index(&sample_index())?;

    let loaded = store.load_project_index()?.expect("expected cached index");
    assert_eq!(loaded.classes.len(), 1);
    Ok(())
}
