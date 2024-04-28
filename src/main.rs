//! Generated types support documentation
#![deny(missing_docs)]
use anyhow::Ok;
use schemars::JsonSchema;
use futures::{pin_mut, TryStreamExt};

use serde::{Deserialize, Serialize};
use tracing::info;
use apiexts::CustomResourceDefinition;
use k8s_openapi::{api::core::v1::{Pod}, apiextensions_apiserver::pkg::apis::apiextensions::v1 as apiexts, serde};

use kube::{
    api::{Api, Patch, PatchParams, ResourceExt,ListParams},
    runtime::wait::{await_condition, conditions},
    runtime::{watcher, watcher::Event},
    Client, CustomResource, CustomResourceExt,
};

#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[kube(
    group="aria3ppp.ir",
    version="v1",
    kind="Topology",
    namespaced,
    status="TopologyStatus",
)]
struct TopologySpec {
    pub name: String,
    pub pods: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
struct TopologyStatus {
    pub is_ok: bool,
}

const crdName: &str = "topologies.aria3ppp.ir";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let client = Client::try_default().await?;

    // Create the CRD
    let top_apply = PatchParams::apply("topology-apply").force();
    let crd_client: Api<CustomResourceDefinition> = Api::all(client.clone());

    crd_client.patch(crdName, &top_apply, &Patch::Apply(Topology::crd())).await?;

    info!("Creating out CRD yay: {}", serde_yaml::to_string(&Topology::crd())?);

    let establish = await_condition(crd_client, crdName, conditions::is_crd_established());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), establish).await?;

    // Get the pods current topology
    let pods: Api<Pod> = Api::default_namespaced(client.clone());

    // Watch the topology resource
    let spec = create_spec(pods).await?;

    let topologies: Api<Topology> = Api::default_namespaced(client.clone());
    let tt = topologies.patch("default", &top_apply, &Patch::Apply(&Topology::new("default", spec))).await?;

    info!("Applied 1 {}: {:?}", tt.name_any(), tt.spec);

    let pods: Api<Pod> = Api::default_namespaced(client.clone());

    let obs = watcher(pods, ListParams::default());
    pin_mut!(obs);
    while let Some(e) = obs.try_next().await? {
        let pods: Api<Pod> = Api::default_namespaced(client.clone());
        let mut spec = create_spec(pods.clone()).await?;

        match e {
            Event::Applied(p) => {
                // On apply
                if !spec.pods.contains(&p.name_any()) {
                    spec.pods.push(p.name_any());
                }
            },
            Event::Deleted(p) => {
                // On delete
                spec.pods.retain(|s| *s!= p.name_any());
            },
            _ => continue,
        }

        let topologies: Api<Topology> = Api::default_namespaced(client.clone());
        let tt = topologies.patch("default", &top_apply, &Patch::Apply((&Topology::new("default", spec)))).await?;
    }

    Ok(())
}

async fn create_spec(pods: Api<Pod>) -> anyhow::Result<TopologySpec> {
    let pod_list = pods.list(&ListParams::default()).await?;

    let top_spec = TopologySpec{
        name: "default".to_owned(),
        pods: pod_list.iter().map(|pod| pod.name_any()).collect(),
    };

    Ok(top_spec)
}