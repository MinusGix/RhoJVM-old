# Build all of java files in classpath

# Cd into classpath so that it gives them the proper path, like java/lang/Class
cd classpath
# CLASSPATH="../rhojvm/ex/lib/rt/" 
J_FILES=""
# java/lang
JFILES="$JFILES ./java/lang/Class.java ./java/lang/System.java ./java/lang/String.java"

# java/lang/reflect
JFILES="$JFILES ./java/lang/reflect/Field.java"

# sun/misc/
JFILES="$JFILES ./sun/misc/Unsafe.java"

# rho/
JFILES="$JFIELS ./rho/StringConversion.java ./rho/InternalField.java"


# Compile them all
javac $JFILES

cd ..