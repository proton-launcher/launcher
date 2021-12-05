package proton;

import java.lang.reflect.Method;
import java.lang.reflect.Field;
import java.security.Permission;
import java.security.AccessControlException;
import java.util.ArrayList;
import java.util.List;
import java.io.File;
import java.util.Scanner;
import java.util.Arrays;
import java.io.FileNotFoundException;
import java.security.AccessController;
import java.security.ProtectionDomain;

public class CustomSecurityManager extends SecurityManager {

    private List<List<String>> allowedPermissions = null;

    @Override
    public void checkPermission(Permission permission) {
        super.checkPermission(permission);
        //check(permission);
    }

//    @Override
//    public void checkPermission(Permission permission, Object context) {
//    }

    private void check(Permission permission) throws AccessControlException {
        //System.out.println(permission);
        if (this.allowedPermissions == null) {
            this.allowedPermissions = new ArrayList<>();

            File file = new File(System.getProperty("security_location"));
            try {
                Scanner scanner = new Scanner(file);
                while (scanner.hasNextLine()) {
                    String line = scanner.nextLine();
                    String[] split = line.split(",");
                    System.out.println(Arrays.toString(split));
                }
            } catch (FileNotFoundException e) {
                e.printStackTrace();
            }
        }
    }

//    private void check(Permission permission) throws AccessControlException {
//        Class[] context = getClassContext();
//        //System.out.println(Arrays.toString(context));
//        for (int i = 2; i < context.length; i++) {
//            if (context[i].getName().equals("proton.CustomSecurityManager")) return;
//        }
//        //System.out.println(Arrays.toString(context));
//        ProtectionDomain[] domains = new ProtectionDomain[context.length];
//        for (int i = 0; i < context.length; i++) {
//            domains[i] = context[i].getProtectionDomain();
//        }
//        //System.out.println(permission);
//        //System.out.println(Arrays.toString(context));
//
//        for (int i = 0; i < context.length; i++) {
//            Class<?> clazz = context[i];
//            String name = clazz.getName();
//            ProtectionDomain domain = domains[i];
//            if (name.startsWith("java.") || name.startsWith("jdk.") || name.startsWith("javax.") || name.startsWith("proton.") || name.startsWith("sun.") || name.startsWith("com.sun.")) continue;
//            //System.out.println(clazz);
//            System.out.println(clazz.getProtectionDomain().getCodeSource().getLocation());
//            if (!domain.implies(permission)) {
//                //System.out.println(domain.getPermissions());
//                System.out.println("failed: " + clazz + " " + domain.getCodeSource().getLocation() + " " + permission);
//                throw new AccessControlException(permission.toString());
//            }
//            return;
//        }
//        System.out.println("----");
//
//        boolean canExecute = false;
//    }

}
