# dogether-contracts
Dogether is a hybrid no loss lottery, the rule is pool your UST Dogether will yield on Anchor and buy tickets for you 
on LoTerra lottery contract with your earning. In other way more you pool more tickets you will get and probably more chance to win the jackpot.
Here this is different from traditional lottery, there is not every time a winner so rich should not come richer.
So what are you waiting =) Dogether now!

---
### Hybrid no loss
- Jackpot come from the lottery contract, this have the advantage to start with `>100K` Jackpot in comparison for a competitor 
  no loss they will need 30Mm UST pooled on Anchor at 20% to get 100K per week jackpot
- Rich don't come richer
---
### Compiling
To compile all the contracts, run the following in the repo root:

```
docker run --rm -v "$(pwd)":/code \
--mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
cosmwasm/workspace-optimizer:0.12.3
```
This will compile all packages in the `contracts` directory and output the stripped and optimized wasm code under the `artifacts` directory as output, along with a `checksums.txt file.

If you hit any issues there and want to debug, you can try to run the following in each contract dir: `RUSTFLAGS="-C link-arg=-s" cargo build --release --target=wasm32-unknown-unknown --locked`


