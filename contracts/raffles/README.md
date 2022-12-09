This raffle contract is explained in details across the different implementation files in the (src)[src] directory.

On thing we need to detail here is how you provide randomness to raffles when they are nearly finished.
At any time during the raffle and before the end of the timeout, anyone can provide a randomness seed to the contract that will be used to draw the result of the raffle when it ends. The randomness has to be provided between the close timestamp of the raffle and the timeout timestamp : 

```rust 
	const close_timestamp = raffle_info
	    .raffle_options
	    .raffle_start_timestamp
	    .plus_seconds(raffle_info.raffle_options.raffle_duration)

	const timeout_timestamp = raffle_info
	    .raffle_options
	    .raffle_start_timestamp
	    .plus_seconds(raffle_info.raffle_options.raffle_duration)
	    .plus_seconds(raffle_info.raffle_options.raffle_timeout)
```
A randomness can still be provided after the timeout timestamp if no other randomness was provided.
We ensure a non-predictable randomness is always provided by having a minimum of 2 minutes of `raffle_timeout` (the drand period is 30s) and having an API that provides randomness as soon as a raffle is closed.


The following script allows one to provide randomness from the drand.love source registered with the contract :
using the  

```typescript

	// Actually this won't work exactly as is because of type enforcement. 
	import { HttpCachingChain, HttpChainClient, fetchBeacon, ChainedBeacon } from "drand-client";

  async getBeacon() {
    const chainHash = "8990e7a9aaed2ffed73dbd7092123d6f289930540d7651336225dc172e51b2ce"; // (hex encoded)
    const publicKey =
      "868f005eb8e6e4ca0a47c8a77ceaa5309a47978a7c71bc5cce96366b5d7a569937c529eeda66c7293784a9402801af31"; // (hex encoded)

    const options = {
      disableBeaconVerification: false, // `true` disables checking of signatures on beacons - faster but insecure!!!
      noCache: true, // `true` disables caching when retrieving beacons for some providers
      chainVerificationParams: { chainHash, publicKey }, // these are optional, but recommended! They are compared for parity against the `/info` output of a given node
    };

    // if you want to connect to a single chain to grab the latest beacon you can simply do the following
    const chain = new HttpCachingChain("https://api.drand.sh", options);
    const client = new HttpChainClient(chain, options);
    return fetchBeacon(client);
  }

  async provideRandomness(raffleId: number){
    const beacon = await this.getBeacon();

	  const executeMsg = {
	    update_randomness: {
	      raffle_id: raffleId,
	      randomness: {
	        round: beacon.round,
	        signature: Buffer.from(beacon.signature, "hex").toString("base64"),
	        previous_signature: Buffer.from(beacon.previous_signature, "hex").toString(
	          "base64",
	        ),
	      },
	    },
	  };


  }

```

If you are the last randomness provider on the contract, you will get a reward when the raffle closes. 
It will be a percentage of the raffle tickets bought by users 