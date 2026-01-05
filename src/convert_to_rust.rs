use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::cmp::Ordering;
use indicatif::{ProgressBar, ProgressStyle};

/// Скалярные типы
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ScalarType {
    String,
    Bool,
    Int,
    Float,
}

/// Тип поля
#[derive(Debug, Clone)]
enum FieldType {
    Scalar(ScalarType),
    Enum(String),
    Array(Box<FieldType>),
    Object(String),
    Any,
}

/// Описание поля структуры
struct FieldDef {
    original_name: String,
    rust_name: String,
    field_type: FieldType,
    optional: bool,
    comment: Option<String>,
    rename_attr: bool,
}

/// Описание структуры
struct StructDef {
    name: String,
    fields: Vec<FieldDef>,
}

/// Описание enum (варианты хранятся как строки, подходит и для строковых, и для смешанных скалярных)
struct EnumDef {
    name: String,
    variants: BTreeSet<String>,
    comment: String,
    is_string_enum: bool, // true — если enum из Vec<String>
}

/// Контекст генерации
pub struct Context {
    skip_comments: BTreeSet<String>,
    rename: BTreeSet<String>,
    structs: Vec<StructDef>,
    enums: Vec<EnumDef>,
    registry: BTreeMap<String, String>,      // signature -> struct name
    enum_registry: BTreeMap<String, String>, // signature -> enum name
    progress:     Option<ProgressBar>,  // ← добавили
}

impl Context {
    /// Создаёт новый контекст
    fn new(skip_comments: &[&str], rename: &[&str]) -> Self {
        Context {
            skip_comments: skip_comments.iter().map(|s| s.to_string()).collect(),
            rename: rename.iter().map(|s| s.to_string()).collect(),
            structs: Vec::new(),
            enums: Vec::new(),
            registry: BTreeMap::new(),
            enum_registry: BTreeMap::new(),
            progress:      None,
        }
    }

    /// Запускает анализ для корневой структуры
    fn build_root(&mut self, root_name: &str, values: &[Value]) {
        // Собираем set из всех ключей root-объектов, чтобы знать общее число полей
        let total_fields = values.iter().filter_map(Value::as_object).map(|m| {m.len()}).max().unwrap_or(100) as u64;
        // создаём единый прогресс-бар
        let pb = ProgressBar::new(total_fields).with_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] {prefix} [{bar:40.cyan/blue}] {pos}/{len} {msg}").expect("invalid template"));
        pb.set_prefix(root_name.to_string());
        self.progress = Some(pb.clone());
        // рекурсивно строим структуру
        self.build_struct(root_name, values);
        // завершаем бар
        pb.finish_with_message("Field analysis is completed!");
    }

    /// Рекурсивно строит struct для объектов и регистрирует его
    fn build_struct(&mut self, struct_name: &str, values: &[Value]) -> String {
        let total = values.len();
        let mut field_map: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        for v in values {
            if let Value::Object(map) = v {
                for (k, vchild) in map {
                    field_map.entry(k.as_str()).or_default().push(vchild);
                }
            }
        }
        let mut fields = Vec::new();
        for (orig, vals) in field_map {
            // если общий прогресс-бар инициализирован — инкрементим его
            if let Some(pb) = &self.progress {pb.set_message(format!("Field processing `{}`", orig)); pb.inc(1);}
            let count = vals.len();
            let optional = count < total || vals.iter().any(|v| v.is_null());
            let ftype = self.determine_field_type(struct_name, &orig, &vals);
            let rust_name = self.compute_rust_name(struct_name, &orig);
            let rename_attr = self.rename.contains(orig) || orig.chars().next().map_or(false, |c| c.is_numeric()) || rust_name != to_snake_case(&orig);
            // Вычисляем комментарий для скалярных полей, одномерных массивов скаляров и массивов enum’ов из Vec<String>
            let comment = match &ftype {
                // 0) Любые многомерные массивы (размерность >1) — только первое значение
                FieldType::Array(inner) if matches!(**inner, FieldType::Array(_)) => {
                    // vals здесь — &[Value], каждый из которых тоже Value::Array
                    if let Some(Value::Array(_arr0)) = vals.get(0) {
                        // arr0 — Vec<Value>, берём его первый элемент
                        if let Some(first_val) = vals.get(0) {
                            // Просто to_string(), т.к. может быть любой Value
                            Some(first_val.to_string())
                        } else {None}
                    } else {None}
                }
                // 1) Обычные скалярные поля
                FieldType::Scalar(_) => {
                    if self.skip_comments.contains(orig) {
                        // берём только первый элемент
                        vals.get(0).map(|v| {if v.is_string() {format!("\"{}\"", v.as_str().unwrap())} else {v.to_string()}})
                    } else {Some(unique_values_summary(&vals))}
                } 
                // 2) Одномерные массивы скалярных значений Vec<T>
                FieldType::Array(inner) if matches!(**inner, FieldType::Scalar(_)) => {
                    if self.skip_comments.contains(orig) {
                        // только первый подмассив
                        if let Some(Value::Array(arr)) = vals.get(0) {
                            let json_array = Value::Array(arr.clone());
                            Some(serde_json::to_string(&json_array).expect("serialization must succeed"))
                        } else {None}
                    } else {
                        // Набираем JSON‑строку каждого подмассива
                        let mut arrs = BTreeSet::new();
                        for v in &vals {
                            if let Value::Array(arr) = v {
                                let json_array = Value::Array(arr.clone());
                                let s = serde_json::to_string(&json_array).expect("не должно падать");
                                arrs.insert(s);
                            }
                        }
                        // Собираем уникальные массивы через запятую
                        Some(arrs.into_iter().collect::<Vec<_>>().join(", "))
                    }
                }
                // 3) Если это именно Enum, порождённый из чистого String поля (is_string_enum)   
                FieldType::Enum(_en) => {
                    /*if let Some(ed) = self.enums.iter().find(|e| &e.name == en && e.is_string_enum) {
                        // вставляем комментарий со всеми уникальными строковыми вариантами
                        Some(ed.comment.clone())
                    } else {None}*/
                    let items: Vec<String> = vals.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    let uniq: Vec<String> = items.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
                    Some(uniq.join(", "))
                }
                // 4) Одномерные массивы enum’ов Vec<SomeEnum>
                FieldType::Array(inner) if matches!(**inner, FieldType::Enum(_)) => {
                    if let FieldType::Enum(en) = &**inner {
                        if let Some(ed) = self.enums.iter().find(|e| &e.name == en) {
                            if self.skip_comments.contains(orig) {
                                // Только первый вариант из comment (разделённого запятыми)
                                /*ed.comment.split(", ").next().map(|s| s.to_string())*/
                                Some(vals.get(0).unwrap_or(&&Value::Null).to_string())
                            } else {Some(ed.comment.clone())}
                        } else {None}
                    } else {None}
                }
                // ничто больше
                _ => None,
            };
            fields.push(FieldDef {original_name: orig.to_string(), rust_name, field_type: ftype, optional, comment, rename_attr,});
        }
        let sig = signature(&fields);
        if let Some(existing) = self.registry.get(&sig) {existing.clone()} else {
            self.registry.insert(sig, struct_name.to_string());
            self.structs.push(StructDef {name: struct_name.to_string(), fields,});
            struct_name.to_string()
        }
    }

    /// Определяет тип поля по списку значений
    fn determine_field_type(&mut self, parent: &str, field: &str, vals: &[&Value]) -> FieldType {
        let non_null: Vec<&Value> = vals.iter().filter(|v| !v.is_null()).copied().collect();
        if non_null.is_empty() {return FieldType::Any;}
        if non_null.iter().all(|v| v.is_string()) {
            // Собираем уникальные строковые варианты
            let vals_set: BTreeSet<String> = non_null.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            // Если уникальных значений от 2 до 9 включительно — делаем enum,
            // иначе — оставляем String
            if (2..=9).contains(&vals_set.len()) && !self.skip_comments.contains(field) {
                // общая подпись
                let sig = format!("StringEnum:{}", vals_set.iter().cloned().collect::<Vec<_>>().join("|"));
                // реюз или создание нового enum-а
                let enum_name = if let Some(name) = self.enum_registry.get(&sig) {name.clone()} else {
                    let name = format!("{}{}Enum", to_upper_camel_case(parent), to_upper_camel_case(field));
                    self.enum_registry.insert(sig.clone(), name.clone());
                    self.enums.push(EnumDef {name: name.clone(), variants: vals_set.clone(), comment: vals_set.iter().cloned().collect::<Vec<_>>().join(", "), is_string_enum: true,});
                    name
                };
                return FieldType::Enum(enum_name);
            } else {
                // слишком мало (0–1) или слишком много (>=10) вариантов — просто String
                return FieldType::Scalar(ScalarType::String);
            }
        }
        if non_null.iter().all(|v| v.is_boolean()) {return FieldType::Scalar(ScalarType::Bool);}
        if non_null.iter().all(|v| v.is_number()) {
            let all_int = non_null.iter().all(|v| v.as_i64().is_some());
            return if all_int {FieldType::Scalar(ScalarType::Int)} else {FieldType::Scalar(ScalarType::Float)};
        }
        // Обработка одномерных массивов
        if non_null.iter().all(|v| v.is_array()) {
            // Собираем все значения из всех массивов
            let mut elems = Vec::new();
            for v in &non_null {
                if let Some(arr) = v.as_array() {
                    //elems.extend(arr.clone());
                    for item in arr {elems.push(item);}
                }
            }
            // Особый случай: одномерный массив строк превращаем в enum
            if elems.iter().all(|v| v.is_string()) {
                // Собираем уникальные строковые варианты
                let variants: BTreeSet<String> = elems.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                // Используем **ту же** подпись
                let sig = format!("StringEnum:{}",variants.iter().cloned().collect::<Vec<_>>().join("|"));
                // Пытаемся переиспользовать
                if let Some(enum_name) = self.enum_registry.get(&sig) {return FieldType::Array(Box::new(FieldType::Enum(enum_name.clone())));}
                // Иначе создаём новый enum
                let enum_name = format!("{}{}Enum", to_upper_camel_case(parent), to_upper_camel_case(field));
                self.enum_registry.insert(sig.clone(), enum_name.clone());
                self.enums.push(EnumDef {name: enum_name.clone(), variants: variants.clone(), comment: variants.iter().cloned().collect::<Vec<_>>().join(", "), is_string_enum: true,});
                return FieldType::Array(Box::new(FieldType::Enum(enum_name)));
            }
            // Обычная обработка вложенных массивов
            let nested_field_name = if field.chars().next().map_or(false, |c| c.is_numeric()) {format!("{}_Elem", parent)} else {format!("{}", field)};
            let inner = self.determine_field_type(parent, &nested_field_name, &elems);
            return FieldType::Array(Box::new(inner));
        }
        if non_null.iter().all(|v| v.is_object()) {
            let nested_name = if field.chars().next().map_or(false, |c| c.is_numeric()) {
                format!("{}Elem", parent)
            } else if self.rename.contains(field) {
                // зарезервированные или переименованные поля → Parent + Field
                format!("{}{}", parent, to_upper_camel_case(field))
            } else {
                to_upper_camel_case(field)
            };
            let nested_vals: Vec<Value> = non_null.into_iter().cloned().collect();
            return FieldType::Object(self.build_struct(&nested_name, &nested_vals));
        }
        if non_null.iter().all(|v| v.is_string() || v.is_boolean() || v.is_number()) {
            // Смешанные скалярные типы → enum с вариантами-строками
            let mut set = BTreeSet::new();
            for v in &non_null {
                if v.is_string() {
                    set.insert(ScalarType::String);
                } else if v.is_boolean() {
                    set.insert(ScalarType::Bool);
                } else if v.is_number() {
                    set.insert(ScalarType::Int);
                    set.insert(ScalarType::Float);
                }
            }
            // подпись для повторного использования одного enum
            let mut codes: Vec<char> = set.iter().map(|st| match st {
                ScalarType::Bool => 'B',
                ScalarType::Int => 'I',
                ScalarType::Float => 'F',
                ScalarType::String => 'S',
            }).collect();
            codes.sort();
            let sig = format!("E{}", codes.iter().collect::<String>());
            // имя enum
            let enum_name = if let Some(n) = self.enum_registry.get(&sig) {n.clone()} else {
                // создаём новый enum
                let name = format!("{}{}Enum", to_upper_camel_case(parent), to_upper_camel_case(field));
                self.enum_registry.insert(sig.clone(), name.clone());
                // конвертируем варианты ScalarType в строки
                let mut variant_strs = BTreeSet::new();
                for st in &set {
                    let vstr = match st {
                        ScalarType::Bool => "Bool".to_string(),
                        ScalarType::Int => "Int".to_string(),
                        ScalarType::Float => "Float".to_string(),
                        ScalarType::String => "String".to_string(),
                    };
                    variant_strs.insert(vstr);
                }
                let summary_vals: Vec<&Value> = non_null.clone();
                self.enums.push(EnumDef { name: name.clone(), variants: variant_strs, comment: unique_values_summary(&summary_vals), is_string_enum: false, });
                name
            };
            return FieldType::Enum(enum_name);
        }
        FieldType::Any
    }
    /// Вычисляет rust-имя поля
    fn compute_rust_name(&self, parent: &str, orig: &str) -> String {
        if self.rename.contains(orig) || orig.chars().next().map_or(false, |c| c.is_numeric()) {format!("{}_{}", to_snake_case(parent), to_snake_case(orig))} else {to_snake_case(orig)}
    }
    /// Генерирует итоговый код (enums + structs)
    fn generate_code(&self, generate_impl_from: bool, impl_source_object: String, enums_import_path: String, ) -> String {
        // создаём прогресс-бар
        let total_tasks = self.enums.iter().map(|e| e.variants.len()).sum::<usize>() + self.structs.iter().map(|s| s.fields.len()).sum::<usize>();
        let pb = ProgressBar::new(total_tasks as u64).with_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] {prefix} [{bar:40.cyan/blue}] {pos}/{len} {msg}").expect("invalid template"));
        pb.set_prefix("Generate_code".to_string());
        let mut out = String::new();
        out.push_str("use serde::{Serialize, Deserialize};\n");
        // Enums
        if !generate_impl_from {
            out.push_str("use strum_macros::Display;\n\n");
            for e in &self.enums {
                out.push_str(&format!("#[derive(Debug, Serialize, Deserialize, Clone, Display{})]\n", (if e.is_string_enum {", Default"} else {""})));
                out.push_str(&format!("pub enum {} {{\n", e.name));
                if e.is_string_enum {out.push_str("\t#[default]\n");}
                for variant in &e.variants {
                    let mut var_name = to_upper_camel_case(variant);
                    if e.is_string_enum {
                        if !var_name.is_empty() {
                            if variant.chars().next().map_or(false, |c| c.is_numeric()) {var_name = format!("Enum{}", var_name);}
                            // старый вариант для строковых enum’ов
                            out.push_str(&format!("\t#[serde(rename = \"{}\")]\n", variant));
                            out.push_str(&format!("\t{},\n", var_name));
                        }
                    } else {
                        // новый вариант для скалярных enum’ов
                        let ty = match variant.as_str() {
                            "Bool"   => "bool",
                            "Int"    => "i64",
                            "Float"  => "f64",
                            "String" => "String",
                            _        => "Value", // на всякий случай
                        };
                        out.push_str(&format!("\t{}({}),\n", var_name, ty));
                    }
                    pb.inc(1);
                }
                out.push_str("}\n\n");
                // implement default for scalar emun
                if !e.is_string_enum {
                    out.push_str(&format!("impl Default for {} {{\n", e.name));
                    out.push_str("\tfn default() -> Self {\n");
                    out.push_str(&format!("\t\t{}::Int(0)\n", e.name));
                    out.push_str("\t}\n");
                    out.push_str("}\n\n");
                }
                //impl
                // добавляем impl From<model_name::Enum> для каждого enum
                /*out.push_str(&format!("impl From<{}::{}> for {} {{\n", base_object, e.name, e.name));
                out.push_str(&format!("\tfn from(obj: {}::{}) -> Self {{\n", base_object, e.name));
                out.push_str("\t\tmatch obj {\n");
                if e.is_string_enum {
                    for variant in &e.variants {
                        let mut var_name = to_upper_camel_case(variant);
                        if variant.chars().next().map_or(false, |c| c.is_numeric()) {var_name = format!("Enum{}", var_name);}
                        out.push_str(&format!("\t\t\t{}::{}::{} => {}::{},\n", base_object,e.name, var_name, e.name, var_name));
                    }
                } else {
                    for variant in &e.variants {
                        let var_name = to_upper_camel_case(variant);
                        out.push_str(&format!("\t\t\t{}::{}::{}(val) => {}::{}(val),\n", base_object,e.name, var_name, e.name, var_name));
                    }
                }
                out.push_str("\t\t}\n");
                out.push_str("\t}\n");
                out.push_str("}\n\n");*/
            }
        } else {
            out.push_str(&format!("{}::{{", enums_import_path));
            for e in &self.enums {
                out.push_str(&format!("{}, ", e.name));
            }
            out.push_str("};\n\n");
        }
        // Structs
        for s in &self.structs {
            out.push_str("#[derive(Debug, Serialize, Deserialize, Default, Clone)]\n");
            out.push_str(&format!("pub struct {} {{\n", s.name));
            for f in &s.fields {
                if f.rename_attr {
                    if f.optional {
                        out.push_str(&format!("\t#[serde(rename = \"{}\", skip_serializing_if = \"Option::is_none\")]\n", f.original_name));
                    } else {
                        out.push_str(&format!("\t#[serde(rename = \"{}\")]\n", f.original_name));
                    }
                } else if f.optional {
                    out.push_str("\t#[serde(skip_serializing_if = \"Option::is_none\")]\n");
                }
                let mut l_comment = String::new();
                if let Some(c) = &f.comment {
                    // а) если это JSON-массив(ы) — начинаются с '['
                    if c.starts_with('[') {
                        // вставляем как есть:
                        l_comment = format!(" /* {} */", c);
                    } else {
                        // б) иначе — это скаляр(ы) или enum — разбиваем и численно сортируем
                        let mut items: Vec<String> = c.clone().split(',').map(|s| s.trim().to_string()).collect();
                        items.sort_by(|a, b| {
                            let a_num = a.parse::<i64>();
                            let b_num = b.parse::<i64>();
                            match (a_num, b_num) {
                                // оба числа — сравниваем по значению
                                (Ok(a_val), Ok(b_val)) => a_val.cmp(&b_val),
                                // только a — число, b — не число → помещаем числа вправо
                                (Ok(_), Err(_)) => Ordering::Greater,
                                // только b — число → помещаем числа влево
                                (Err(_), Ok(_)) => Ordering::Less,
                                // оба не числа — чистый лексикографический порядок
                                (Err(_), Err(_)) => a.cmp(b),
                            }
                        });
                        l_comment = format!(" /* {} */", items.join(", "));
                    }
                }
                let ty = type_to_rust(&f.field_type, f.optional);
                out.push_str(&format!("\tpub {}: {}{},\n", f.rust_name, ty, l_comment));
                pb.inc(1);
            }
            out.push_str("}\n\n");

            if generate_impl_from {
                // добавляем impl From<model_name::Struct> для каждой структуры
                out.push_str(&format!("impl From<{}::{}> for {} {{\n", impl_source_object, s.name, s.name));
                out.push_str(&format!("\tfn from(obj: {}::{}) -> Self {{\n", impl_source_object, s.name));
                out.push_str(&format!("\t\t{} {{\n", s.name));
                for f in &s.fields {
                    // определяем присвоение в зависимости от типа
                    let ty = type_to_rust(&f.field_type, f.optional);
                    let assign = {
                        // вспомогательная функция для проверки скаляра по имени
                        fn is_scalar_type(name: &str) -> bool {matches!(name, "bool" | "i64" | "f64" | "String" | "Value")}

                        if ty.starts_with("Option<") {
                            let inner = &ty["Option<".len()..ty.len() - 1];
                            if inner.starts_with("Vec<") {
                                let mut inner2 = &inner["Vec<".len()..inner.len() - 1];
                                while inner2.starts_with("Vec<") && inner2.ends_with('>') {inner2 = &inner2[4..inner2.len()-1];}
                                if is_scalar_type(inner2) {
                                    format!("obj.{}", f.rust_name)
                                } else {
                                    if ty.starts_with("Vec<Vec<") {format!("obj.{}.map(|vec| vec.into_iter().map(|inner_vec| {{inner_vec.into_iter().map(Into::into).collect()}}).collect())",f.rust_name)
                                    } else {format!("obj.{}.map(|vec| vec.into_iter().map(Into::into).collect())",f.rust_name)}
                                }
                            } else if is_scalar_type(inner) {
                                format!("obj.{}", f.rust_name)
                            } else {
                                format!("obj.{}.map(Into::into)", f.rust_name)
                            }
                        } else if ty.starts_with("Vec<") {
                            let mut inner = &ty["Vec<".len()..ty.len() - 1];
                            while inner.starts_with("Vec<") && inner.ends_with('>') {inner = &inner[4..inner.len()-1];}
                            if is_scalar_type(inner) {
                                format!("obj.{}", f.rust_name)
                            } else {
                                if ty.starts_with("Vec<Vec<") {format!("obj.{}.into_iter().map(|inner_vec| {{inner_vec.into_iter().map(Into::into).collect()}}).collect()",f.rust_name)
                                } else {format!("obj.{}.into_iter().map(Into::into).collect()",f.rust_name)}
                            }
                        } else if is_scalar_type(&ty) {
                            format!("obj.{}", f.rust_name)
                        } else {
                            format!("obj.{}.into()", f.rust_name)
                        }

                    };
                    out.push_str(&format!("\t\t\t{}: {},\n", f.rust_name, assign));
                }
                out.push_str("\t\t}\n");
                out.push_str("\t}\n");
                out.push_str("}\n\n");
            }
        }
        pb.finish_with_message("Code generation complete!");
        out
    }    
}

/// Вспомогательная функция: генерирует полный код из параметров
pub fn generate_structs(root_name: &str, transactions: &[Value], skip_comments: &[&str], rename: &[&str], generate_impl_from: bool, impl_source_object: String, enums_import_path: String, ) -> String {
    let mut ctx = Context::new(skip_comments, rename);
    ctx.build_root(&to_upper_camel_case(&capitalize(root_name)), transactions);
    ctx.generate_code(generate_impl_from, impl_source_object, enums_import_path)
}

fn unique_values_summary(vals: &[&Value]) -> String {
    // BTreeSet для быстрого `contains` без порядка
    let mut seen = BTreeSet::new();
    // Vec<String> для сохранения именно тех строк, которые мы хотим вывести
    let mut items = Vec::new();

    for v in vals {
        // готовим строку только один раз
        let s = if v.is_string() {
            // для строковых значений берем без дополнительных кавычек
            v.as_str().unwrap().to_string()
        } else {
            // для всего остального — JSON-представление
            v.to_string()
        };
        // если такого ещё не было — запоминаем
        if seen.insert(s.clone()) {
            items.push(s);
        }
    }
    items.join(", ")
}

/// Формирует сигнатуру struct по полям и их типам
fn signature(fields: &[FieldDef]) -> String {
    let mut parts = Vec::new();
    for f in fields {
        let tsig = match &f.field_type {
            FieldType::Scalar(st) => format!("S{:?}", st),
            FieldType::Enum(name) => format!("E{}", name),
            FieldType::Array(inner) => format!("A[{}]", type_sig(inner)),
            FieldType::Object(name) => format!("O{}", name),
            FieldType::Any => "Any".to_string(),
        };
        let opt = if f.optional {"?"} else {""};
        parts.push(format!("{}:{}{}", f.original_name, tsig, opt));
    }
    parts.sort();
    parts.join("|")
}

/// Сигнатура FieldType для подписи
fn type_sig(ft: &FieldType) -> String {
    match ft {
        FieldType::Scalar(st) => format!("S{:?}", st),
        FieldType::Enum(name) => format!("E{}", name),
        FieldType::Array(inner) => format!("A[{}]", type_sig(inner)),
        FieldType::Object(name) => format!("O{}", name),
        FieldType::Any => "Any".to_string(),
    }
}

/// Преобразует FieldType в строку Rust-типа, учитывая Option
fn type_to_rust(ft: &FieldType, optional: bool) -> String {
    let base = match ft {
        FieldType::Scalar(ScalarType::String) => "String".to_string(),
        FieldType::Scalar(ScalarType::Bool) => "bool".to_string(),
        FieldType::Scalar(ScalarType::Int) => "i64".to_string(),
        FieldType::Scalar(ScalarType::Float) => "f64".to_string(),
        FieldType::Enum(name) => name.clone(),
        FieldType::Array(inner) => format!("Vec<{}>", type_to_rust(inner, false)),
        FieldType::Object(name) => name.clone(),
        FieldType::Any => "Value".to_string(),
    };
    if optional {
        format!("Option<{}>", base)
    } else {
        base
    }
}

/// Преобразует строку в snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            for lc in ch.to_lowercase() {
                result.push(lc);
            }
        } else if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push('_');
        }
    }
    result
}

/// Преобразует строку в UpperCamelCase
fn to_upper_camel_case(s: &str) -> String {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                let mut w = String::new();
                w.extend(first.to_uppercase());
                for ch in chars {
                    w.extend(ch.to_lowercase());
                }
                w
            } else {
                String::new()
            }
        })
        .collect()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}