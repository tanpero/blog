use kuchiki::{NodeRef};
use markup5ever::QualName;
use markup5ever::ns;
use markup5ever::namespace_url;

pub fn process_footnote(_document: &NodeRef) -> NodeRef {

    let document = _document.clone();

    let footnote_definitions: Vec<_> = document.select(".footnote-definition")
                                        .unwrap().collect();

    let container = match document.select(".container")
                            .unwrap().next() {
        Some(container) => container,
        None => return document,
    };

    if footnote_definitions.is_empty() {
        return document;
    }

    let temp_container = NodeRef::new_element(
                            QualName::new(None, ns!(html),
                                "div".into()),
                            None);
    temp_container.as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("style", String::from("margin: 80px"));

    for footnote in footnote_definitions {
        temp_container.append(footnote.as_node().clone());
    }

    container.as_node().append(temp_container);
    
    document

}
