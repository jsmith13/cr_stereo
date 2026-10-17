#include <stddef.h>
#include <stdint.h>

enum capture_transform {
    CAPTURE_MONO = 0,
    CAPTURE_LEFT = 1,
    CAPTURE_RIGHT = 2,
};

static int16_t average_pair(int16_t left, int16_t right)
{
    int32_t sum = (int32_t)left + (int32_t)right;

    if (sum >= 0) {
        return (int16_t)(sum / 2);
    }
    return (int16_t)-(((-sum) + 1) / 2);
}

/*
 * Compacts an interleaved capture block in place. The caller must reject an
 * odd final DSP word count before committing a WAV file.
 */
int capture_transform_in_place(
    int16_t *words,
    size_t word_count,
    enum capture_transform transform,
    size_t *frame_count)
{
    size_t frames;
    size_t i;

    if (words == NULL || frame_count == NULL || (word_count & 1U) != 0U) {
        return -1;
    }
    if (transform != CAPTURE_MONO &&
        transform != CAPTURE_LEFT &&
        transform != CAPTURE_RIGHT) {
        return -2;
    }

    frames = word_count / 2U;
    for (i = 0; i < frames; ++i) {
        int16_t left = words[i * 2U];
        int16_t right = words[i * 2U + 1U];

        if (transform == CAPTURE_MONO) {
            words[i] = average_pair(left, right);
        } else if (transform == CAPTURE_LEFT) {
            words[i] = left;
        } else {
            words[i] = right;
        }
    }

    *frame_count = frames;
    return 0;
}
