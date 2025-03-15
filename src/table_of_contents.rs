use kuchiki::{parse_html, traits::*, NodeRef};
use markup5ever::QualName;
use std::collections::HashMap;
use markup5ever::ns;
use markup5ever::namespace_url;

pub fn enable_table_of_contents(_document: &NodeRef) -> NodeRef {   

    let document = _document.clone();

    let headings: Vec<NodeRef> = document
        .select("h1, h2, h3, h4, h5, h6")
        .unwrap()
        .map(|node| node.as_node().clone())
        .collect();

    if headings.is_empty() {
        return document;
    }

    let mut counters: HashMap<u8, u32> = HashMap::new();

    // 遍历标题，生成编号并设置 ID
    for heading in &headings {
        let tag_name = heading.as_element().unwrap().name.local.to_string();
        let level = tag_name.chars().nth(1).unwrap().to_digit(10).unwrap() as u8;

        // 重置更高层级的计数器
        for i in level + 1..=6 {
            counters.insert(i, 0);
        }

        // 更新当前层级的计数器
        let count = counters.entry(level).or_insert(0);
        *count += 1;

        // 生成编号
        let mut number = String::new();
        for i in 1..=level {
            number.push_str(&counters.get(&i).unwrap_or(&0).to_string());
            number.push('.');
        }
        number.pop(); // 移除最后一个点

        // 设置标题 ID
        let anchor_id = format!("{}-{}", tag_name, number);
        heading
            .as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("id", anchor_id.to_string());
    }

    // 创建目录容器
    let toc = NodeRef::new_element(
        QualName::new(None, ns!(html), "div".into()),
        None,
    );
    toc.as_element()
        .unwrap()
        .attributes
        .borrow_mut()
        .insert("id", "table-of-contents".to_string());
    toc.as_element()
        .unwrap()
        .attributes
        .borrow_mut()
        .insert("class", "table-of-contents".to_string());

    let toc_list = NodeRef::new_element(
        QualName::new(None, ns!(html), "ul".into()),
        None,
    );
    toc_list
        .as_element()
        .unwrap()
        .attributes
        .borrow_mut()
        .insert("class", "toc-list".to_string());
    toc.append(toc_list.clone());

    // 当前目录项栈
    let mut current_toc_items = vec![toc_list.clone()];

    // 遍历标题，生成目录项
    for heading in &headings {
        let tag_name = heading.as_element().unwrap().name.local.to_string();
        let level = tag_name.chars().nth(1).unwrap().to_digit(10).unwrap() as u8;

        // 创建目录项
        let list_item = NodeRef::new_element(
            QualName::new(None, ns!(html), "li".into()),
            None,
        );
        list_item
            .as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("class", format!("toc-level-{}", level).to_string());

        let link = NodeRef::new_element(
            QualName::new(None, ns!(html), "a".into()),
            None,
        );
        link.as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("href", format!("#{}", heading.as_element().unwrap().attributes.borrow().get("id").unwrap()).to_string());
        link.append(NodeRef::new_text(heading.text_contents().trim()));
        list_item.append(link);

        // 调整当前目录项栈
        while current_toc_items.len() > level as usize {
            current_toc_items.pop();
        }

        // 处理子目录
        if level > 1 {
            let last_item = current_toc_items.last().unwrap();
            if last_item.select("ul").unwrap().next().is_none() {
                let sub_list = NodeRef::new_element(
                    QualName::new(None, ns!(html), "ul".into()),
                    None,
                );
                last_item.append(sub_list.clone());
            }
            let sub_list = last_item.select("ul").unwrap().next().unwrap().as_node().clone();
            current_toc_items.push(sub_list);
        }

        // 将目录项添加到当前层级
        current_toc_items.last().unwrap().append(list_item);
    }

    // 添加折叠按钮
    if let Some(first_toc_item) = toc_list.select("li").unwrap().next() {
        let toggle_button = NodeRef::new_element(
            QualName::new(None, ns!(html), "button".into()),
            None,
        );
        toggle_button
            .as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("class", "toggle-btn".to_string());
        toggle_button.append(NodeRef::new_text("折叠目录 👇"));
        toggle_button
            .as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("style", "margin-right: 10px; padding: 5px 10px; cursor: pointer; font-size: 1em;".to_string());

        toggle_button
            .as_element()
            .unwrap()
            .attributes
            .borrow_mut()
            .insert("onclick", r#"
                const subLists = document.querySelectorAll('.table-of-contents ul');
                if (this.textContent.includes('折叠')) {
                    subLists.forEach(subList => {
                        subList.style.display = 'none';
                    });
                    this.textContent = '展开目录 👆';
                } else {
                    subLists.forEach(subList => {
                        subList.style.display = 'block';
                    });
                    this.textContent = '折叠目录 👇';
                }
            "#.to_string());

        // 插入按钮
        first_toc_item
            .as_node()
            .parent()
            .unwrap()
            .insert_before(toggle_button);
    }

    // 将目录插入到文档中
    if let Some(_container) = document.select(".container").unwrap().next() {
        if let Some(first_heading) = document.select("h1").unwrap().next() {
            first_heading.as_node().insert_after(toc.clone());
        }
    }

    document
}
