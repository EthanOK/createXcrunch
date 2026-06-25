/*
 * B20 vanity address miner kernel for createXcrunch.
 * Hashes abi.encode(deployer, salt) — 64 bytes — with keccak256.
 */

#define OPENCL_PLATFORM_UNKNOWN 0
#define OPENCL_PLATFORM_AMD   2

#ifndef PLATFORM
# define PLATFORM       OPENCL_PLATFORM_UNKNOWN
#endif

#if PLATFORM == OPENCL_PLATFORM_AMD
# pragma OPENCL EXTENSION   cl_amd_media_ops : enable
#endif

typedef union _nonce_t
{
  ulong   uint64_t;
  uint    uint32_t[2];
  uchar   uint8_t[8];
} nonce_t;

#if PLATFORM == OPENCL_PLATFORM_AMD
static inline ulong ROL(const ulong x, const uint s)
{
  uint2 output;
  uint2 x2 = as_uint2(x);
  output = (s > 32u) ? amd_bitalign((x2).yx, (x2).xy, 64u - s) : amd_bitalign((x2).xy, (x2).yx, 32u - s);
  return as_ulong(output);
}
#else
#define ROL(X, S) (((X) << S) | ((X) >> (64 - S)))
#endif

#define THETA_(M, N, O) t = b[M] ^ ROL(b[N], 1); \
a[O + 0] = a[O + 0] ^ t; a[O + 5] = a[O + 5] ^ t; a[O + 10] = a[O + 10] ^ t; \
a[O + 15] = a[O + 15] ^ t; a[O + 20] = a[O + 20] ^ t;

#define THETA() \
b[0] = a[0] ^ a[5] ^ a[10] ^ a[15] ^ a[20]; \
b[1] = a[1] ^ a[6] ^ a[11] ^ a[16] ^ a[21]; \
b[2] = a[2] ^ a[7] ^ a[12] ^ a[17] ^ a[22]; \
b[3] = a[3] ^ a[8] ^ a[13] ^ a[18] ^ a[23]; \
b[4] = a[4] ^ a[9] ^ a[14] ^ a[19] ^ a[24]; \
THETA_(4, 1, 0); THETA_(0, 2, 1); THETA_(1, 3, 2); THETA_(2, 4, 3); THETA_(3, 0, 4);

#define RHO_PI_(M, N) t = b[0]; b[0] = a[M]; a[M] = ROL(t, N);

#define RHO_PI() t = a[1]; b[0] = a[10]; a[10] = ROL(t, 1); \
RHO_PI_(7, 3); RHO_PI_(11, 6); RHO_PI_(17, 10); RHO_PI_(18, 15); RHO_PI_(3, 21); RHO_PI_(5, 28); \
RHO_PI_(16, 36); RHO_PI_(8, 45); RHO_PI_(21, 55); RHO_PI_(24, 2); RHO_PI_(4, 14); RHO_PI_(15, 27); \
RHO_PI_(23, 41); RHO_PI_(19, 56); RHO_PI_(13, 8); RHO_PI_(12, 25); RHO_PI_(2, 43); RHO_PI_(20, 62); \
RHO_PI_(14, 18); RHO_PI_(22, 39); RHO_PI_(9, 61); RHO_PI_(6, 20); RHO_PI_(1, 44);

#define CHI_(N) \
b[0] = a[N + 0]; b[1] = a[N + 1]; b[2] = a[N + 2]; b[3] = a[N + 3]; b[4] = a[N + 4]; \
a[N + 0] = b[0] ^ ((~b[1]) & b[2]); \
a[N + 1] = b[1] ^ ((~b[2]) & b[3]); \
a[N + 2] = b[2] ^ ((~b[3]) & b[4]); \
a[N + 3] = b[3] ^ ((~b[4]) & b[0]); \
a[N + 4] = b[4] ^ ((~b[0]) & b[1]);

#define CHI() CHI_(0); CHI_(5); CHI_(10); CHI_(15); CHI_(20);

#define IOTA(X) a[0] = a[0] ^ X;

#define ITER(X) THETA(); RHO_PI(); CHI(); IOTA(X);

#define ITERS() \
ITER(0x0000000000000001); ITER(0x0000000000008082); \
ITER(0x800000000000808a); ITER(0x8000000080008000); \
ITER(0x000000000000808b); ITER(0x0000000080000001); \
ITER(0x8000000080008081); ITER(0x8000000000008009); \
ITER(0x000000000000008a); ITER(0x0000000000000088); \
ITER(0x0000000080008009); ITER(0x000000008000000a); \
ITER(0x000000008000808b); ITER(0x800000000000008b); \
ITER(0x8000000000008089); ITER(0x8000000000008003); \
ITER(0x8000000000008002); ITER(0x8000000000000080); \
ITER(0x000000000000800a); ITER(0x800000008000000a); \
ITER(0x8000000080008081); ITER(0x8000000000008080); \
ITER(0x0000000080000001); ITER(0x8000000080008008);

static inline void keccakf(ulong *a)
{
  ulong b[5];
  ulong t;
  ITERS();
}

static inline bool isMatching(uchar const *d)
{
  __constant char* pattern = PATTERN();

#pragma unroll
  for (uint i = 0; i < 20; ++i) {
    uchar byte = d[i];
    char highNibble = (byte >> 4) & 0x0F;
    char lowNibble = byte & 0x0F;
    char highChar = (highNibble < 10) ? ('0' + highNibble) : ('a' + highNibble - 10);
    char lowChar = (lowNibble < 10) ? ('0' + lowNibble) : ('a' + lowNibble - 10);
    char patternHighChar = pattern[2 * i];
    char patternLowChar = pattern[2 * i + 1];
    if (patternHighChar != 'X' && patternHighChar != highChar)
      return false;
    if (patternLowChar != 'X' && patternLowChar != lowChar)
      return false;
  }
  return true;
}

#define hasSuffixTotal(d) ( \
  (!(d[0])) + (!(d[1])) + (!(d[2])) + (!(d[3])) + \
  (!(d[4])) + (!(d[5])) + (!(d[6])) + (!(d[7])) + \
  (!(d[8])) \
>= TOTAL_ZEROES)

static inline bool hasSuffixLeading(uchar const *d)
{
#pragma unroll
  for (uint i = 0; i < LEADING_ZEROES; ++i) {
    if (d[i] != 0) return false;
  }
  return true;
}

__kernel void hashB20Salt(
  __constant uchar const *d_message,
  __constant uint const *d_nonce,
  __global volatile ulong *restrict solutions
) {
  ulong spongeBuffer[25];
#define sponge ((uchar *) spongeBuffer)
#define digest (sponge)

  nonce_t nonce;
  nonce.uint32_t[0] = get_global_id(0);
  nonce.uint32_t[1] = d_nonce[0];

#pragma unroll
  for (int i = 0; i < 12; ++i)
    sponge[i] = 0;

  sponge[12] = DEPLOY_0;
  sponge[13] = DEPLOY_1;
  sponge[14] = DEPLOY_2;
  sponge[15] = DEPLOY_3;
  sponge[16] = DEPLOY_4;
  sponge[17] = DEPLOY_5;
  sponge[18] = DEPLOY_6;
  sponge[19] = DEPLOY_7;
  sponge[20] = DEPLOY_8;
  sponge[21] = DEPLOY_9;
  sponge[22] = DEPLOY_10;
  sponge[23] = DEPLOY_11;
  sponge[24] = DEPLOY_12;
  sponge[25] = DEPLOY_13;
  sponge[26] = DEPLOY_14;
  sponge[27] = DEPLOY_15;
  sponge[28] = DEPLOY_16;
  sponge[29] = DEPLOY_17;
  sponge[30] = DEPLOY_18;
  sponge[31] = DEPLOY_19;

  sponge[32] = DEPLOY_0;
  sponge[33] = DEPLOY_1;
  sponge[34] = DEPLOY_2;
  sponge[35] = DEPLOY_3;
  sponge[36] = DEPLOY_4;
  sponge[37] = DEPLOY_5;
  sponge[38] = DEPLOY_6;
  sponge[39] = DEPLOY_7;
  sponge[40] = DEPLOY_8;
  sponge[41] = DEPLOY_9;
  sponge[42] = DEPLOY_10;
  sponge[43] = DEPLOY_11;
  sponge[44] = DEPLOY_12;
  sponge[45] = DEPLOY_13;
  sponge[46] = DEPLOY_14;
  sponge[47] = DEPLOY_15;
  sponge[48] = DEPLOY_16;
  sponge[49] = DEPLOY_17;
  sponge[50] = DEPLOY_18;
  sponge[51] = DEPLOY_19;
  sponge[52] = 0;
  sponge[53] = d_message[0];
  sponge[54] = d_message[1];
  sponge[55] = d_message[2];
  sponge[56] = d_message[3];
  sponge[57] = nonce.uint8_t[0];
  sponge[58] = nonce.uint8_t[1];
  sponge[59] = nonce.uint8_t[2];
  sponge[60] = nonce.uint8_t[3];
  sponge[61] = nonce.uint8_t[4];
  sponge[62] = nonce.uint8_t[5];
  sponge[63] = nonce.uint8_t[6];

  sponge[64] = 0x01u;

#pragma unroll
  for (int i = 65; i < 135; ++i)
    sponge[i] = 0;

  sponge[135] = 0x80u;

#pragma unroll
  for (int i = 136; i < 200; ++i)
    sponge[i] = 0;

  keccakf(spongeBuffer);

  uchar addr[20];
  addr[0] = 0xB2u;
#pragma unroll
  for (int i = 1; i < 10; ++i)
    addr[i] = 0;
  addr[10] = VARIANT_BYTE;
#pragma unroll
  for (int i = 0; i < 9; ++i)
    addr[11 + i] = digest[i];

  if (SUCCESS_CONDITION()) {
    solutions[0] = nonce.uint64_t;

    ulong newUint64 = 0;
#pragma unroll
    for (ulong i = 0; i < 8; i++) {
      ulong d = addr[i];
      newUint64 |= (d << ((7 - i) * 8));
    }
    solutions[1] = newUint64;

    newUint64 = 0;
#pragma unroll
    for (ulong j = 0; j < 8; j++) {
      ulong d = addr[j + 8];
      newUint64 |= (d << ((7 - j) * 8));
    }
    solutions[2] = newUint64;

    newUint64 = 0;
#pragma unroll
    for (ulong k = 0; k < 4; k++) {
      ulong d = addr[16 + k];
      newUint64 |= (d << ((7 - k) * 8));
    }
    solutions[3] = newUint64;
  }
}
