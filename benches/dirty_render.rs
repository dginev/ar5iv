extern crate criterion;

use ar5iv::assemble_asset::assemble_paper;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use rocket::tokio::runtime::Runtime;

fn dirty_prepare_bench(c: &mut Criterion) {
  let runtime = Runtime::new().unwrap();
  
  c.bench_function("assemble dirty with regex", move |b| {
    b.to_async(&runtime).iter(|| async { 
      assemble_paper(None, None, "2105.04026", false).await
     })
  });
}

fn dom_prepare_bench(c: &mut Criterion) {
  let runtime = Runtime::new().unwrap();
  
  c.bench_function("assemble with dom", move |b| {
    b.to_async(&runtime).iter(|| async { 
      assemble_paper(None, None, "2105.04026", true).await
     })
  });
}


criterion_group!(benches, dirty_prepare_bench, dom_prepare_bench);
criterion_main!(benches);
