# Copyright (c) 2019 Karl Sundequist Blomdahl <karl.sundequist.blomdahl@gmail.com>
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

from cffi import FFI

def load_shared_library(ffi):
    for library_name in ['./libgo.so', './go.dll']:
        try:
            return ffi.dlopen(library_name)
        except:
            pass

    print('Failed to load the shared library -- libgo.so')
    quit(1)

# -------- Simple FFI (independent functions) --------

SIMPLE_FFI = FFI()
SIMPLE_FFI.cdef("""
    int get_num_features();
""")

SIMPLE_LIB = load_shared_library(SIMPLE_FFI)

def get_num_features():
    """ Returns the number of features that the Go engine will use. """
    return SIMPLE_LIB.get_num_features()

# -------- Complex FFI (depends on the number of features) --------

COMPLEX_FFI = FFI()
COMPLEX_FFI.cdef("""
    typedef struct {
        float features[""" + str(361 * get_num_features()) + """];
        int index;
        int color;
        char policy[905];
        int winner;
        int number;
    } Example;

    int extract_single_example(const char*, Example*);
""")

COMPLEX_LIB = load_shared_library(COMPLEX_FFI)
FEATURE_SIZE = get_num_features() * 361 * COMPLEX_FFI.sizeof('float')

def get_single_example(line):
    """ Returns a single example, from the given SGF file. """
    raw_example = COMPLEX_FFI.new('Example[]', 1)
    result = COMPLEX_LIB.extract_single_example(line, raw_example)

    if result == 0:
        example = {
            '_raw_example': raw_example,  # prevent it from being garbage collected
            'features': COMPLEX_FFI.buffer(raw_example[0].features, FEATURE_SIZE),
            'color': raw_example[0].color,
            'index': raw_example[0].index,
            'policy': COMPLEX_FFI.string(raw_example[0].policy),
            'winner': raw_example[0].winner,
            'number': raw_example[0].number
        }
    else:
        example = None

    return result, example