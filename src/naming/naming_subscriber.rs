#![allow(unused_imports)]

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use actix::prelude::*;

use super::{
    model::ServiceKey,
    naming_delay_nofity::{DelayNotifyActor, DelayNotifyCmd},
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ListenerClusterType {
    All,
    One(Arc<String>),
}

impl Default for ListenerClusterType {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct ListenerKey {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub cluster: ListenerClusterType,
}

#[derive(Debug, Clone)]
pub struct NamingListenerItem {
    pub service_key: ServiceKey,
    pub clusters: Option<HashSet<String>>,
}

#[derive(Default)]
pub struct Subscriber {
    listener: HashMap<ServiceKey, HashMap<Arc<String>, Option<HashSet<String>>>>,
    client_keys: HashMap<Arc<String>, HashSet<ServiceKey>>,
    notify_addr: Option<Addr<DelayNotifyActor>>,
}

impl Subscriber {
    pub fn new() -> Self {
        Self {
            listener: Default::default(),
            client_keys: Default::default(),
            notify_addr: Default::default(),
        }
    }

    pub fn set_notify_addr(&mut self, notify_addr: Addr<DelayNotifyActor>) {
        self.notify_addr = Some(notify_addr);
    }

    pub fn add_subscribe(&mut self, client_id: Arc<String>, items: Vec<NamingListenerItem>) {
        match self.client_keys.get_mut(&client_id) {
            Some(set) => {
                for item in &items {
                    set.insert(item.service_key.clone());
                }
            }
            None => {
                let mut set = HashSet::new();
                for item in &items {
                    set.insert(item.service_key.clone());
                }
                self.client_keys.insert(client_id.clone(), set);
            }
        }
        for item in items {
            match self.listener.get_mut(&item.service_key) {
                Some(set) => {
                    set.insert(client_id.clone(), item.clusters);
                }
                None => {
                    let mut set = HashMap::new();
                    set.insert(client_id.clone(), item.clusters);
                    self.listener.insert(item.service_key, set);
                }
            };
        }
    }

    pub fn remove_subscribe(&mut self, client_id: Arc<String>, items: Vec<NamingListenerItem>) {
        let mut remove_keys = vec![];
        for item in &items {
            if let Some(set) = self.listener.get_mut(&item.service_key) {
                set.remove(&client_id);
                if set.is_empty() {
                    remove_keys.push(item.service_key.clone());
                }
            };
        }
        for key in &remove_keys {
            self.listener.remove(key);
        }

        let mut remove_empty_client = false;
        if let Some(set) = self.client_keys.get_mut(&client_id) {
            for item in items {
                set.remove(&item.service_key);
            }
            if set.is_empty() {
                remove_empty_client = true;
            }
        };
        if remove_empty_client {
            self.client_keys.remove(&client_id);
        }
    }

    pub fn remove_client_subscribe(&mut self, client_id: Arc<String>) {
        if let Some(set) = self.client_keys.remove(&client_id) {
            let mut remove_keys = vec![];
            for key in set {
                if let Some(set) = self.listener.get_mut(&key) {
                    set.remove(&client_id);
                    if set.is_empty() {
                        remove_keys.push(key);
                    }
                }
            }
            for key in &remove_keys {
                self.listener.remove(key);
            }
        }
    }

    pub fn remove_key(&mut self, key: ServiceKey) {
        if let Some(set) = self.listener.remove(&key) {
            let mut remove_keys = vec![];
            for (client_id, _) in set {
                if let Some(set) = self.client_keys.get_mut(&client_id) {
                    set.remove(&key);
                    if set.is_empty() {
                        remove_keys.push(client_id);
                    }
                }
            }
            for key in &remove_keys {
                self.client_keys.remove(key);
            }
        }
    }

    pub fn notify(&self, key: ServiceKey) {
        //log::info!("naming_subscriber notify {:?}",&key);
        if let Some(notify_addr) = &self.notify_addr {
            if let Some(set) = self.listener.get(&key) {
                let mut client_id_set = HashSet::new();
                for item in set.keys() {
                    client_id_set.insert(item.clone());
                }
                notify_addr.do_send(DelayNotifyCmd::Notify(key, client_id_set));
            }
        }
    }

    pub fn get_listener_key_size(&self) -> usize {
        self.listener.len()
    }

    pub fn get_listener_value_size(&self) -> usize {
        let mut sum = 0;
        for map in self.listener.values() {
            sum += map.len();
        }
        sum
    }

    pub fn get_client_size(&self) -> usize {
        self.client_keys.len()
    }

    pub fn get_client_value_size(&self) -> usize {
        let mut sum = 0;
        for item in self.client_keys.values() {
            sum += item.len();
        }
        sum
    }

    pub fn fuzzy_match_listener(
        &self,
        group_name: &str,
        service_name: &str,
        namespace_id: &str,
    ) -> HashMap<ServiceKey, HashMap<Arc<String>, Option<HashSet<String>>>> {
        self.listener
            .iter()
            .filter(|(key, _)| {
                key.group_name.contains(group_name)
                    && key.service_name.contains(service_name)
                    && key.namespace_id.contains(namespace_id)
            }) // 模糊匹配
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}
