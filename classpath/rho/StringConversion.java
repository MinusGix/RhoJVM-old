package rho;

import java.nio.CharBuffer;
import java.nio.ByteBuffer;
import java.nio.charset.Charset;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CoderResult;
import java.util.Arrays;

/// Internal class for converting charsets
public class StringConversion {
    /// Convert the data into a specific charset
    public static char[] convertToChars(byte data[], int offset, int length, Charset charset) throws CharacterCodingException {
        // Create a decoder for converting to the given charset
        CharsetDecoder decoder = charset.newDecoder()
            // If we can't map the character then replace it
            .onUnmappableCharacter(CodingErrorAction.REPLACE)
            // If the input is bad then replace it
            .onMalformedInput(CodingErrorAction.REPLACE)
            .reset();
        // Decode the input with the given charset
        CharBuffer output = decoder.decode(ByteBuffer.wrap(data, offset, length));
        // Return the char[] version of it
        return output.array();
    }

    public static byte[] convertFromChars(char data[], int offset, int length, Charset charset) throws CharacterCodingException {
        CharsetEncoder encoder = charset.newEncoder()
            // If we can't map the character then replace it
            .onUnmappableCharacter(CodingErrorAction.REPLACE)
            // If the input is bad then replace it
            .onMalformedInput(CodingErrorAction.REPLACE)
            .reset();
        
        // We can overallocate the necessary space so that we only have to allocate once, sometimes
        double max_bytes = encoder.maxBytesPerChar();
        int enough = (int) ((double)length * max_bytes);
        
        byte[] out_array = new byte[enough];

        ByteBuffer out = ByteBuffer.wrap(out_array);
        CharBuffer in = CharBuffer.wrap(data, offset, length);

        // TODO: Check for errors
        CoderResult res = encoder.encode(in, out, true);
        
        // TODO: We could have a custom function to 'cut up' an array
        // that optionally copies the data if there's more than one existent reference
        // which would avoid an allocation if we're the only reference
        // or that could be a generic optimization
        if (enough == out.position()) {
            return out_array;
        } else {
            return Arrays.copyOf(out_array, out.position());
        }
    }
}