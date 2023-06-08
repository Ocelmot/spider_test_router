use std::{io, path::PathBuf};

use spider_client::{
    message::{
        DatasetMessage, DatasetPath, Message, UiElement,
        UiElementContent, UiElementContentPart, UiElementKind, UiMessage, UiPageManager, UiPath, UiInput, RouterMessage, DatasetData,
    },
    AddressStrategy, Relation, Role, SpiderClient, SpiderId2048,
};

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    println!("Hello, world!");

    let client_path = PathBuf::from("client_state.dat");
    let mut client = if client_path.exists() {
        SpiderClient::from_file(&client_path)
    } else {
        let mut client = SpiderClient::new();
        client.set_state_path(&client_path);
        client.add_strat(AddressStrategy::Addr(String::from("localhost:1930")));
        client.save();
        client
    };

    if !client.has_host_relation() {
        let path = PathBuf::from("spider_keyfile.json");

        let data = match std::fs::read_to_string(&path) {
            Ok(str) => str,
            Err(_) => String::from("[]"),
        };
        let id: SpiderId2048 = serde_json::from_str(&data).expect("Failed to deserialize spiderid");
        let host = Relation {
            id,
            role: Role::Peer,
        };
        client.set_host_relation(host);
        client.save();
    }

    client.connect().await;
    let mut state = State::init(&mut client).await;

    loop {
        match client.recv().await {
            Some(msg) => state.msg_handler(&mut client, msg).await,
            None => break, //  done!
        }
    }

    Ok(())
}

struct State {
    recps: Vec<DatasetData>,
    msgs: Vec<DatasetData>,
}

impl State {
    async fn init(client: &mut SpiderClient) -> Self {
        // let id = client.self_relation().id;

        let msg = RouterMessage::SetIdentityProperty("name".into(), "Test Router".into());
        let msg = Message::Router(msg);
        client.send(msg).await;

        // Subscribe to recp dataset
        let recp_dataset = DatasetPath::new_private(vec![String::from("Recp")]);
        let msg = Message::Dataset(DatasetMessage::Subscribe {
            path: recp_dataset.clone(),
        });
        client.send(msg).await;

        // Subscribe to Msgs dataset
        let msgs_dataset = DatasetPath::new_private(vec![String::from("Messages")]);
        let msg = Message::Dataset(DatasetMessage::Subscribe {
            path: msgs_dataset.clone(),
        });
        client.send(msg).await;

        // Subscribe to test_event
        let msg = Message::Router(RouterMessage::Subscribe(String::from("test_event")));
        client.send(msg).await;


        // Setup Page
        let id = client.self_relation().id;
        let mut test_page = UiPageManager::new(id.clone(), "Router Test Page");
        let mut root = test_page
            .get_element_mut(&UiPath::root())
            .expect("all pages have a root");
        root.set_kind(UiElementKind::Rows);

        root.append_child({
            let mut element = UiElement::from_string("Add Recp");
            element.set_kind(UiElementKind::TextEntry);
            element.set_selectable(true);
            element.set_id("Add Recp");
            element
        });

        root.append_child({
            let mut element = UiElement::new(UiElementKind::Rows);
            element.set_dataset(Some(recp_dataset.clone().resolve(id.clone())));
            element.append_child({

                let mut child = UiElement::new(UiElementKind::Text);
                let mut content = UiElementContent::new();
                content.add_part(UiElementContentPart::Data(vec![]));
                child.set_content(content);

                child
            });
            element
        });


        root.append_child({
            let mut element = UiElement::from_string("Send Msg");
            element.set_kind(UiElementKind::TextEntry);
            element.set_selectable(true);
            element.set_id("Send Msg");
            element
        });

        root.append_child({
            let mut element = UiElement::new(UiElementKind::Rows);
            element.set_dataset(Some(msgs_dataset.clone().resolve(id.clone())));
            element.append_child({

                let mut child = UiElement::new(UiElementKind::Text);
                let mut content = UiElementContent::new();
                content.add_part(UiElementContentPart::Data(vec![]));
                child.set_content(content);

                child
            });
            element
        });
        

        drop(root);

        test_page.get_changes(); // clear changes to synch, since we are going to send the whole page at first. This
                                 // Could instead set the initial elements with raw and then recalculate ids
        let msg = Message::Ui(UiMessage::SetPage(test_page.get_page().clone()));
        client.send(msg).await;

        // Create self
        Self {
            recps: vec![],
            msgs: vec![],
        }
    }

    async fn msg_handler(&mut self, client: &mut SpiderClient, msg: Message) {
        match msg {
            Message::Ui(msg) => self.ui_handler(client, msg).await,
            Message::Dataset(msg) => self.dataset_handler(client, msg).await,
            Message::Router(msg) => self.router_handler(client, msg).await,
        }
    }

    async fn dataset_handler(&mut self, client: &mut SpiderClient, msg: DatasetMessage) {
        println!("Message: {:?}", msg);
        if let DatasetMessage::Dataset { path, data } = msg {
            let recp_dataset = DatasetPath::new_private(vec![String::from("Recp")]);
            let msgs_dataset = DatasetPath::new_private(vec![String::from("Messages")]);
            if path == recp_dataset {
                self.recps = data;
            }else if path == msgs_dataset {
                self.msgs = data;
                if self.msgs.len() > 10 {
                    let msgs_dataset = DatasetPath::new_private(vec![String::from("Messages")]);
                    let msg = Message::Dataset(DatasetMessage::DeleteElement {
                        path: msgs_dataset.clone(),
                        id: 0,
                    });
                    client.send(msg).await;
                }
            }
        }
    }

    async fn ui_handler(&mut self, client: &mut SpiderClient, msg: UiMessage) {
        match msg {
            UiMessage::Subscribe => {}
            UiMessage::Pages(_) => {}
            UiMessage::GetPage(_) => {}
            UiMessage::Page(_) => {}
            UiMessage::UpdateElementsFor(_, _) => {}
            UiMessage::InputFor(_, _, _, _) => {}
            UiMessage::SetPage(_) => {}
            UiMessage::ClearPage => {}
            UiMessage::UpdateElements(_) => {}
            UiMessage::Input(element_id, _dataset_ids, change) => {
                match element_id.as_str() {
                    "Add Recp" => {
                        if let UiInput::Text(text) = change{
                            let recp_dataset = DatasetPath::new_private(vec![String::from("Recp")]);
                            let data = spider_client::message::DatasetData::String(text);
                            let msg = Message::Dataset(DatasetMessage::Append {
                                path: recp_dataset,
                                data: data,
                            });
                            client.send(msg).await;
                        }
                    },
                    "Send Msg" => {
                        // emit message
                        if let UiInput::Text(text) = change{
                            // generate recps from data
                            let mut recps = vec![];
                            for recp in &self.recps{
                                if let DatasetData::String(recp) = recp{
                                    if let Some(relation) = Relation::peer_from_base_64(recp){
                                        recps.push(relation);
                                    }
                                }
                            }
                            let data = spider_client::message::DatasetData::String(text);
                            let msg = Message::Router(RouterMessage::SendEvent(
                                String::from("test_event"), 
                                recps,
                                data
                            ));
                            client.send(msg).await;
                        }
                    },
                    _ => return,
                }
            }
            UiMessage::Dataset(_, _) => {}
        }
    }

    async fn router_handler(&mut self, client: &mut SpiderClient, msg: RouterMessage) {
        match msg {
            RouterMessage::SendEvent(_, _, _) =>{}
            RouterMessage::Event(name, _, data) => {
                if name == "test_event" {
                    let msgs_dataset = DatasetPath::new_private(vec![String::from("Messages")]);
                    let msg = Message::Dataset(DatasetMessage::Append {
                        path: msgs_dataset,
                        data: data,
                    });
                    client.send(msg).await;
                }
            },
            RouterMessage::Subscribe(_) => {},
            RouterMessage::Unsubscribe(_) => {},

            // directory messages
            RouterMessage::SubscribeDir => {},
            RouterMessage::UnsubscribeDir => {},
            RouterMessage::AddIdentity(_) => {},
            RouterMessage::RemoveIdentity(_) => {},
            RouterMessage::SetIdentityProperty(_, _) => {},
        }
    }
}
