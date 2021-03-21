/*
 * This implementation of PRS compression/decompression comes from here:
 * https://github.com/Sylverant/libsylverant/blob/67074b719e5b52e6cf55898578e40c2dbccc0839/src/utils/prs.c
 *
 * The reason it is being brought back out into use instead of using
 * libsylverant's current PRS compression/decompression implementation (which is
 * by far the cleanest one out there in my opinion) is due to apparent
 * incompatibilities when used to generate Gamecube download quest .qst files.
 *
 * When using libsylverant's implementation, some generated .qst files work fine.
 * But some others were resulting in black-screen crashes when loaded from the
 * memory card. As soon as I switched to this older PRS implementation to
 * generate the same .qst file (using the same source .bin/.dat files in both
 * cases, obviously) the black-screen crashes disappeared.
 *
 * The externally accessible functions, prefixed "fuzziqer_" so as not to
 * conflict with libsylverant (which I am still using for encryption), are just
 * libsylverant-API-compatible wrappers over the original "prs_" functions found
 * in this source file. This will make it easier to switch away from this older
 * implementation in the future if a fix is found for these apparent
 * incompatibilities.
 *
 * March 2021, Gered King. Original header comment from 2011 follows.
 */
/*
    The PRS compressor/decompressor that this file implements to was originally
    written by Fuzziqer Software. The file was distributed with the message that
    it could be used in anything/for any purpose as long as credit was given. I
    have incorporated it into libsylverant for use with the Sylverant PSO server
    and related utilities.

    Other than minor changes (making it compile cleanly as C) this file has been
    left relatively intact from its original distribution, which was obtained on
    June 21st, 2009 from http://www.fuzziqersoftware.com/files/prsutil.zip

    Modified June 30, 2011 by Lawrence Sebald:
    Make the code work properly when compiled for a 64-bit target.
*/

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <malloc.h>

#include "fuzziqer_prs.h"

////////////////////////////////////////////////////////////////////////////////

typedef struct {
	uint8_t bitpos;
	uint8_t *controlbyteptr;
	uint8_t *srcptr_orig;
	uint8_t *dstptr_orig;
	uint8_t *srcptr;
	uint8_t *dstptr;
} PRS_COMPRESSOR;

static void prs_put_control_bit(PRS_COMPRESSOR *pc, uint8_t bit) {
	*pc->controlbyteptr = *pc->controlbyteptr >> 1;
	*pc->controlbyteptr |= ((!!bit) << 7);
	pc->bitpos++;
	if (pc->bitpos >= 8) {
		pc->bitpos = 0;
		pc->controlbyteptr = pc->dstptr;
		pc->dstptr++;
	}
}

static void prs_put_control_bit_nosave(PRS_COMPRESSOR *pc, uint8_t bit) {
	*pc->controlbyteptr = *pc->controlbyteptr >> 1;
	*pc->controlbyteptr |= ((!!bit) << 7);
	pc->bitpos++;
}

static void prs_put_control_save(PRS_COMPRESSOR *pc) {
	if (pc->bitpos >= 8) {
		pc->bitpos = 0;
		pc->controlbyteptr = pc->dstptr;
		pc->dstptr++;
	}
}

static void prs_put_static_data(PRS_COMPRESSOR *pc, uint8_t data) {
	*pc->dstptr = data;
	pc->dstptr++;
}

static uint8_t prs_get_static_data(PRS_COMPRESSOR *pc) {
	uint8_t data = *pc->srcptr;
	pc->srcptr++;
	return data;
}

////////////////////////////////////////////////////////////////////////////////

static void prs_init(PRS_COMPRESSOR *pc, const void *src, void *dst) {
	pc->bitpos = 0;
	pc->srcptr = (uint8_t *) src;
	pc->srcptr_orig = (uint8_t *) src;
	pc->dstptr = (uint8_t *) dst;
	pc->dstptr_orig = (uint8_t *) dst;
	pc->controlbyteptr = pc->dstptr;
	pc->dstptr++;
}

static void prs_finish(PRS_COMPRESSOR *pc) {
	prs_put_control_bit(pc, 0);
	prs_put_control_bit(pc, 1);

	if (pc->bitpos != 0) {
		*pc->controlbyteptr = ((*pc->controlbyteptr << pc->bitpos) >> 8);
	}

	prs_put_static_data(pc, 0);
	prs_put_static_data(pc, 0);
}

static void prs_rawbyte(PRS_COMPRESSOR *pc) {
	prs_put_control_bit_nosave(pc, 1);
	prs_put_static_data(pc, prs_get_static_data(pc));
	prs_put_control_save(pc);
}

static void prs_shortcopy(PRS_COMPRESSOR *pc, int offset, uint8_t size) {
	size -= 2;
	prs_put_control_bit(pc, 0);
	prs_put_control_bit(pc, 0);
	prs_put_control_bit(pc, (size >> 1) & 1);
	prs_put_control_bit_nosave(pc, size & 1);
	prs_put_static_data(pc, offset & 0xFF);
	prs_put_control_save(pc);
}

static void prs_longcopy(PRS_COMPRESSOR *pc, int offset, uint8_t size) {
	uint8_t byte1, byte2;
	if (size <= 9) {
		prs_put_control_bit(pc, 0);
		prs_put_control_bit_nosave(pc, 1);
		prs_put_static_data(pc, ((offset << 3) & 0xF8) | ((size - 2) & 0x07));
		prs_put_static_data(pc, (offset >> 5) & 0xFF);
		prs_put_control_save(pc);
	} else {
		prs_put_control_bit(pc, 0);
		prs_put_control_bit_nosave(pc, 1);
		prs_put_static_data(pc, (offset << 3) & 0xF8);
		prs_put_static_data(pc, (offset >> 5) & 0xFF);
		prs_put_static_data(pc, size - 1);
		prs_put_control_save(pc);
	}
}

static void prs_copy(PRS_COMPRESSOR *pc, int offset, uint8_t size) {
	if ((offset > -0x100) && (size <= 5)) {
		prs_shortcopy(pc, offset, size);
	} else {
		prs_longcopy(pc, offset, size);
	}
	pc->srcptr += size;
}

////////////////////////////////////////////////////////////////////////////////

static uint32_t prs_compress(const void *source, void *dest, uint32_t size) {
	PRS_COMPRESSOR pc;
	int x, y, z;
	uint32_t xsize;
	int lsoffset, lssize;
	uint8_t *src = (uint8_t *) source, *dst = (uint8_t *) dest;
	prs_init(&pc, source, dest);

	for (x = 0; x < size; x++) {
		lsoffset = lssize = xsize = 0;
		for (y = x - 3; (y > 0) && (y > (x - 0x1FF0)) && (xsize < 255); y--) {
			xsize = 3;
			if (!memcmp(src + y, src + x, xsize)) {
				do xsize++;
				while (!memcmp(src + y, src + x, xsize) &&
				       (xsize < 256) &&
				       ((y + xsize) < x) &&
				       ((x + xsize) <= size)
						);
				xsize--;
				if (xsize > lssize) {
					lsoffset = -(x - y);
					lssize = xsize;
				}
			}
		}
		if (lssize == 0) {
			prs_rawbyte(&pc);
		} else {
			prs_copy(&pc, lsoffset, lssize);
			x += (lssize - 1);
		}
	}
	prs_finish(&pc);
	return pc.dstptr - pc.dstptr_orig;
}

////////////////////////////////////////////////////////////////////////////////

static uint32_t prs_decompress(const void *source, void *dest) // 800F7CB0 through 800F7DE4 in mem
{
	uint32_t r0, r3, r6, r9; // 6 unnamed registers
	uint32_t bitpos = 9; // 4 named registers 
	uint8_t *sourceptr = (uint8_t *) source;
	uint8_t *sourceptr_orig = (uint8_t *) source;
	uint8_t *destptr = (uint8_t *) dest;
	uint8_t *destptr_orig = (uint8_t *) dest;
	uint8_t *ptr_reg;
	uint8_t currentbyte;
	int flag;
	int32_t offset;
	uint32_t x, t; // 2 placed variables

	currentbyte = sourceptr[0];
	sourceptr++;
	for (;;) {
		bitpos--;
		if (bitpos == 0) {
			currentbyte = sourceptr[0];
			bitpos = 8;
			sourceptr++;
		}
		flag = currentbyte & 1;
		currentbyte = currentbyte >> 1;
		if (flag) {
			destptr[0] = sourceptr[0];
			sourceptr++;
			destptr++;
			continue;
		}
		bitpos--;
		if (bitpos == 0) {
			currentbyte = sourceptr[0];
			bitpos = 8;
			sourceptr++;
		}
		flag = currentbyte & 1;
		currentbyte = currentbyte >> 1;
		if (flag) {
			r3 = sourceptr[0] & 0xFF;
			offset = ((sourceptr[1] & 0xFF) << 8) | r3;
			sourceptr += 2;
			if (offset == 0) return (uint32_t) (destptr - destptr_orig);
			r3 = r3 & 0x00000007;
			//r5 = (offset >> 3) | 0xFFFFE000;
			if (r3 == 0) {
				flag = 0;
				r3 = sourceptr[0] & 0xFF;
				sourceptr++;
				r3++;
			} else r3 += 2;
			//r5 += (uint32_t)destptr;
			ptr_reg = destptr + ((int32_t) ((offset >> 3) | 0xFFFFE000));
		} else {
			r3 = 0;
			for (x = 0; x < 2; x++) {
				bitpos--;
				if (bitpos == 0) {
					currentbyte = sourceptr[0];
					bitpos = 8;
					sourceptr++;
				}
				flag = currentbyte & 1;
				currentbyte = currentbyte >> 1;
				offset = r3 << 1;
				r3 = offset | flag;
			}
			offset = sourceptr[0] | 0xFFFFFF00;
			r3 += 2;
			sourceptr++;
			//r5 = offset + (uint32_t)destptr;
			ptr_reg = destptr + offset;
		}
		if (r3 == 0) continue;
		t = r3;
		for (x = 0; x < t; x++) {
			//destptr[0] = *(uint8_t*)r5;
			//r5++;
			*destptr++ = *ptr_reg++;
			r3++;
			//destptr++;
		}
	}
}

static uint32_t prs_decompress_size(const void *source) {
	uint32_t r0, r3, r6, r9; // 6 unnamed registers
	uint32_t bitpos = 9; // 4 named registers 
	uint8_t *sourceptr = (uint8_t *) source;
	uint8_t *destptr = NULL;
	uint8_t *destptr_orig = NULL;
	uint8_t *ptr_reg;
	uint8_t currentbyte, lastbyte;
	int flag;
	int32_t offset;
	uint32_t x, t; // 2 placed variables

	currentbyte = sourceptr[0];
	sourceptr++;
	for (;;) {
		bitpos--;
		if (bitpos == 0) {
			lastbyte = currentbyte = sourceptr[0];
			bitpos = 8;
			sourceptr++;
		}
		flag = currentbyte & 1;
		currentbyte = currentbyte >> 1;
		if (flag) {
			sourceptr++;
			destptr++;
			continue;
		}
		bitpos--;
		if (bitpos == 0) {
			lastbyte = currentbyte = sourceptr[0];
			bitpos = 8;
			sourceptr++;
		}
		flag = currentbyte & 1;
		currentbyte = currentbyte >> 1;
		if (flag) {
			r3 = sourceptr[0];
			offset = (sourceptr[1] << 8) | r3;
			sourceptr += 2;
			if (offset == 0) return (uint32_t) (destptr - destptr_orig);
			r3 = r3 & 0x00000007;
			//r5 = (offset >> 3) | 0xFFFFE000;
			if (r3 == 0) {
				r3 = sourceptr[0];
				sourceptr++;
				r3++;
			} else r3 += 2;
			//r5 += (uint32_t)destptr;
			ptr_reg = destptr + ((int32_t) ((offset >> 3) | 0xFFFFE000));
		} else {
			r3 = 0;
			for (x = 0; x < 2; x++) {
				bitpos--;
				if (bitpos == 0) {
					lastbyte = currentbyte = sourceptr[0];
					bitpos = 8;
					sourceptr++;
				}
				flag = currentbyte & 1;
				currentbyte = currentbyte >> 1;
				offset = r3 << 1;
				r3 = offset | flag;
			}
			offset = sourceptr[0] | 0xFFFFFF00;
			r3 += 2;
			sourceptr++;
			//r5 = offset + (uint32_t)destptr;
			ptr_reg = destptr + offset;
		}
		if (r3 == 0) continue;
		t = r3;
		for (x = 0; x < t; x++) {
			//r5++;
			ptr_reg++;
			r3++;
			destptr++;
		}
	}
}

////////////////////////////////////////////////////////////////////////////////

// borrowed from libsylverant: https://github.com/Sylverant/libsylverant/blob/master/src/utils/prs-comp.c
static size_t prs_max_compressed_size(size_t len) {
	len += 2;
	return len + (len >> 3) + ((len & 0x07) ? 1 : 0);
}

/*
 * The below functions are included as wrappers for the above "prs_" functions in order to provide
 * API compatibility with libsylverant's PRS functions, with the goal being to make it easier to
 * switch back to that implementation of PRS compression/decompression in the future. Do note that
 * the error handling is NOT as robust as libsylverant's PRS functions!
 */

int fuzziqer_prs_compress(const uint8_t *src, uint8_t **dst, size_t src_len) {
	if (!src || !dst)
		return -EFAULT;

	if (!src_len)
		return -EINVAL;

	if (src_len < 3)
		return -EBADMSG;

	/* Allocate probably more than enough space for the compressed output. */
	uint8_t *temp_dst;
	size_t max_compressed_size = prs_max_compressed_size(src_len);
	if (!(temp_dst = (uint8_t *)malloc(max_compressed_size)))
		return -errno;

	/* TODO: this version of prs_compress doesn't really do much in the way of error checking ... */
	uint32_t size = prs_compress(src, temp_dst, src_len);

	/* Resize the output (if realloc fails to resize it, then just use the
	   unshortened buffer). */
	if(!(*dst = realloc(temp_dst, size)))
		*dst = temp_dst;

	return size;
}

int fuzziqer_prs_decompress_buf(const uint8_t *src, uint8_t **dst, size_t src_len) {
	if (!src || !dst)
		return -EFAULT;

	if (!src_len)
		return -EINVAL;

	/* The minimum length of a PRS compressed file (if you were to "compress" a
	   zero-byte file) is 3 bytes. If we don't have that, then bail out now. */
	if (src_len < 3)
		return -EBADMSG;

	uint32_t dst_len = prs_decompress_size(src);
	if (!(*dst = malloc(dst_len)))
		return -errno;

	/* TODO: this version of prs_decompress doesn't really do much in the way of error checking ... */
	uint32_t size = prs_decompress(src, *dst);

	return size;
}

int fuzziqer_prs_decompress_size(const uint8_t *src, size_t src_len) {
	if (!src)
		return -EFAULT;

	if (!src_len)
		return -EINVAL;

	/* The minimum length of a PRS compressed file (if you were to "compress" a
	   zero-byte file) is 3 bytes. If we don't have that, then bail out now. */
	if(src_len < 3)
		return -EBADMSG;

	return prs_decompress_size(src);
}
