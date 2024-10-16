# The ultimate blazingly SLOW Rust FFT implementation :fire:

![Image of the code running](./assets/demo.png)

## About :computer:
This repo is meant to be a learning resource to learn the basics of FFT. I coded a simple FFT implementation used in the real world example of decomposing an audio into its frequencies.

First it gets the samples directly from the microphone in real time and then it performs the FFT to get the frequencies.

After having the frequencies, it creates a window with the raw dog sdl2 and then creates a graph and plots the resulting frequencies as well as their contribution to the final audio.


## Libraries :rocket:
1. [Cpal.rs](https://crates.io/crates/cpal) For audio capturing
2. [Sdl2.rs](https://crates.io/crates/sdl2) For drawing graphics

## How to run :clipboard:
First you need to install the sdl2. You can take a look at their [crates.io](https://crates.io/crates/sdl2#requirements) to install it.

On Ubuntu you can just run:
```bash
sudo apt-get install libsdl2-dev
```

Then you need to install the [Cpal.rs](https://crates.io/crates/sdl2) dependencies, take a look at their [crates.io](https://crates.io/crates/cpal).
On Ubuntu you can just run:
```bash
sudo apt install libasound2-dev
```
After installing the dependencies you can run it with:
```bash
cargo run
```


## Discrete Fourier Transform (DFT)
The Discrete Fourier Transform (DFT) is a mathematical operation that transform a discrete-time signal into frequency domain.

Basically just imagine that you captured some evenly spaced values of a function, like the image below:

<div align="center">

![Image of a graph containing evenly spaced values of a function f(x)](./docs/evenly_spaced_values_of_function.jpg)

</div>

Any function can be written as a sum of sines and cosines. And every sum of sines and cosines have a frequency, an amplitude and a phase. 

Our human ear understand each frequency as a musical note (pitch) and that's how we are able to hear and understand music. For example, the note A has a frequency of 440 Hz which means that every second the sum of sines and cosines that generate the note A will oscillate going up and down 440 times every second.

Unfortunately, our computers don't do it automagically. In order for the computer to be able to decompose some function(x) and understand that there is the note A in there or any other frequency, we need to perform a Fourier Transform.

But, there is a problem. We don't know the exact function for any sound captured in the microphone. So, instead of calculating the Fourier Transform, we will be calculating the Discrete Fourier Transform, which basically means that we'll be using evenly spaced values instead of a function as the input to the algorithm.

You can [click here](/docs/DFT.md) to learn more about how the DFT algorithm works, and its mathematical explanation as well as its formula.

## Fast Fourier Transform (FFT)
The only problem with the DFT is that it has a time complexity of n², which basically means that if we double the number of input values (n) this algorithm will take 4 times to run. If we multiply the number of values by 3, the time it takes to run this algorithm will be 9 times higher, and so on...

Even with a modern computer it would be too slow to run this algorithm and, because of that, things would be much slower.

But, we are lucky that there is a specific way to calculate the discrete Fourier transform. This specific way to calculate the DFT has a time complexity of nlog(n) and it's called Fast Fourier Transform or FFT for short.

You can [click here](docs/FFT.md) to understand the specifics of the FFT algorithm, its formula and its mathematical explanation.
