package rho;

import java.nio.CharBuffer;
import java.nio.ByteBuffer;
import java.nio.charset.Charset;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.CharacterCodingException;

/// Internal class for converting charsets
public class StringConversion {
    /// Convert the data into a specific charset
    public static char[] convertToChars(byte data[], int offset, int length, Charset charset) throws CharacterCodingException {
        // Create a decoder for converting to the given charset
        CharsetDecoder decoder = charset.newDecoder()
            // If we can't map the character then replace it
            .onUnmappableCharacter(CodingErrorAction.REPLACE)
            // If the input is bad then replace it
            .onMalformedInput(CodingErrorAction.REPLACE);
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
            .onMalformedInput(CodingErrorAction.REPLACE);
        ByteBuffer output = encoder.encode(CharBuffer.wrap(data, offset, length));
        return output.array();
    }
}