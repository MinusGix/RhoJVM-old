# Build all of java files in classpath

# Cd into classpath so that it gives them the proper path, like java/lang/Class
cd classpath
# CLASSPATH="../rhojvm/ex/lib/rt/" 
J_FILES=""
# java/lang
JFILES="$JFILES ./java/lang/Object.java ./java/lang/ClassLoader.java ./java/lang/Class.java ./java/lang/Package.java ./java/lang/System.java ./java/lang/Runtime.java ./java/lang/Thread.java ./java/lang/String.java ./java/lang/StringBuilder.java ./java/lang/Throwable.java ./java/lang/Integer.java ./java/lang/Long.java ./java/lang/Float.java ./java/lang/Double.java"
JFILE="$JFILES ./java/lang/UnsupportedOperationException.java"

# java/lang/reflect
JFILES="$JFILES ./java/lang/reflect/Field.java"

# java/lang/ref
JFILES="$JFILES ./java/lang/ref/Reference.java"

# java/security
JFILES="$JFILES ./java/security/AccessController.java"

# sun/misc/
JFILES="$JFILES ./sun/misc/VM.java ./sun/misc/Unsafe.java"

# rho/
JFILES="$JFILES ./rho/SystemClassLoader.java ./rho/StringConversion.java ./rho/InternalField.java"

echo $JFILES
# Compile them all
javac -cp . $JFILES

cd ..