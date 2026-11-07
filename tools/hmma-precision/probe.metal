// One 8 x 8 x 8 product D = A * B + C per SIMD-group, with half A, B and C.
// D16 is the half product. D32 is the product with a float accumulator, as the HMMA body of nxb-xlate uses it.
#include <metal_stdlib>
using namespace metal;

kernel void mma(device const half* A [[buffer(0)]], device const half* B [[buffer(1)]], device const half* C [[buffer(2)]],
                device half* D16 [[buffer(3)]], device float* D32 [[buffer(4)]], uint i [[threadgroup_position_in_grid]])
{
    simdgroup_half8x8 a, b, c;
    simdgroup_load(a, A + i * 64, 8);
    simdgroup_load(b, B + i * 64, 8);
    simdgroup_load(c, C + i * 64, 8);
    simdgroup_float8x8 d;
    simdgroup_multiply_accumulate(d, a, b, c);
    simdgroup_store(d, D32 + i * 64, 8);
    simdgroup_multiply_accumulate(c, a, b, c);
    simdgroup_store(c, D16 + i * 64, 8);
}
