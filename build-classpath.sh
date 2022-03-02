# Build all of java files in classpath

# Cd into classpath so that it gives them the proper path, like java/lang/Class
cd classpath
# CLASSPATH="../rhojvm/ex/lib/rt/" 
# java/lang
javac ./java/lang/Class.java
javac ./java/lang/System.java

# sun/misc/
javac ./sun/misc/Unsafe.java

cd ..