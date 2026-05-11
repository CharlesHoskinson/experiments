//! Turmoil test for initial leader election.

mod common;

#[test]
fn single_leader_emerges() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.client("client", async move {
        let _leader = common::leader_url().await;
        Ok(())
    });
    sim.run()
}
